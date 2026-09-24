//! Configuracao do servidor, lida do `config.json`.
//!
//! O arquivo e JSON puro, lido pelo leitor do proprio projeto -- nenhuma
//! dependencia externa entra so por causa da configuracao.

#[cfg(test)]
use crate::apoio_teste::DirTemp;
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

/// Porta padrao do webservice REST. Ver [`Rest`].
pub const PORTA_REST_PADRAO: u16 = 6000;

/// Porta padrao do explorador da especificacao (o "Swagger"). Ver [`Rest`].
///
/// Separada da do REST por decisao do dono, e a separacao e o que torna
/// possivel subir o webservice numa placa SEM abrir a porta do visualizador:
/// sao dois ouvintes, cada um com o proprio interruptor.
pub const PORTA_SWAGGER_PADRAO: u16 = 7000;

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

/// Esta porta atende ALGUEM DE FORA desta maquina?
///
/// Serve ao aviso do arranque das portas HTTP, e por isso responde a pergunta
/// do operador -- "isto esta exposto?" --, nao a pergunta do `SocketAddr`.
/// Sao tres respostas diferentes e so a primeira e obvia:
///
/// - `127.0.0.1` e `[::1]`: laco local, nao atende de fora. `false`.
/// - `0.0.0.0` e `[::]`: o NAO especificado, que e justamente "toda placa
///   desta maquina" -- o caso mais exposto de todos, e o que `is_loopback`
///   sozinho deixaria passar como se fosse endereco qualquer.
/// - endereco de placa (`10.0.0.7`, um IP publico): atende de fora. `true`.
///
/// Nome que nao resolve devolve `false` DE PROPOSITO: quem recusa o endereco
/// invalido e o [`Config::validar`], com o erro que nomeia o campo. Um aviso
/// aqui em cima dele seria um segundo texto sobre a mesma linha errada, e o
/// primeiro que a pessoa lesse seria o menos util.
fn escuta_fora_da_maquina(bind: &str) -> bool {
    match endereco_de(bind) {
        Ok(e) => !e.ip().is_loopback(),
        Err(_) => false,
    }
}

/// Resolve UM caminho do `config.json`: se for relativo, mora ao lado do
/// PROPRIO `config_em` -- nunca do diretorio de trabalho de quem subiu o
/// processo. Absoluto fica exatamente como esta.
///
/// A funcao UNICA para todo campo de caminho do `Config` (pedido 225):
/// `base`, `log_acessos`, `blacklist`, `dblink`, `jobs`, `backup.destino` (em
/// `Config::ler`) e `cifra_fio.arquivo` (sob demanda, em
/// [`CifraFio::caminho_da_chave`]). Antes, cada campo repetia o mesmo
/// `if is_relative() { dir.join(..) }` a mao -- ou, pior, nem repetia: foi
/// assim que `dblink` e `jobs` ficaram de fora, porque entraram no `Config`
/// depois deste bloco ja existir e ninguem voltou para incluir os dois.
fn resolver_caminho_do_config(caminho: &Path, config_em: Option<&Path>) -> PathBuf {
    if caminho.is_absolute() {
        return caminho.to_path_buf();
    }
    match config_em.and_then(Path::parent) {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(caminho),
        // Sem config_em (Config montado sem `ler`), ou config.json escrito
        // sem diretorio (`--config config.json`, que ja mora no cwd): o
        // caminho fica relativo, e resolve contra o cwd do processo -- o que
        // e CERTO no segundo caso (cwd == diretorio do config) e o unico
        // comportamento possivel no primeiro.
        _ => caminho.to_path_buf(),
    }
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

// ---------------------------------------------------------------------------
// A cifra das SAIDAS -- as conexoes que este servidor ABRE
// ---------------------------------------------------------------------------

/// O padrao dos QUATRO interruptores de saida: `replicacao.origens[].cifra`,
/// `cluster.cifra`, `web.servidores[].cifra` e `dblink[].cifra`.
///
/// O quarto entrou em 22/09/2026 (pedido 378), e esta lista ja envelheceu
/// UMA vez por ser digitada: ela dizia tres enquanto o DbLink nascia. Quem
/// acrescentar o quinto conta de novo aqui, nos dois lugares.
///
/// # Por que LIGADO, desde 18/09/2026
///
/// Ordem do dono: *"a comunicacao deve obrigatoriamente ser cifrada"*. O
/// pedido 370 fechou a ENTRADA -- `cifra_fio.exigir` nasce `true` e as portas
/// HTTP recusam o claro -- e mediu o que sobrou de fora: com a entrada
/// exigindo e as saidas nascendo desligadas, **um source de fabrica recusa uma
/// replica de fabrica**, e a suite fica verde do mesmo jeito, porque as
/// bancadas escrevem o escape dos dois lados. A ordem alcanca as duas
/// direcoes; esta constante e a segunda metade dela.
///
/// `"cifra": false` e o escape ESCRITO -- o mesmo molde do `"exigir": false` e
/// do `CIFRA=0` do ODBC: quem quer claro escreve, em vez de esquecer.
///
/// # O custo, dito sem enfeite
///
/// Uma replica desta versao **deixa de falar** com um source anterior ao
/// aperto de mao: aquele servidor nao atende o `cifrar`, e a conexao para.
/// Quem precisa da transicao escreve o escape naquela saida. O custo foi
/// aceito pelo dono junto com a ordem, e o arranque o diz enquanto ninguem
/// escreveu decisao nenhuma (ver [`LeituraDasSaidas`]).
///
/// # Por que ele mora AQUI, e nao dentro de cada leitor
///
/// Num lugar so, citado pelo nome nos quatro leitores -- e no IRMAO que mora
/// fora deste arquivo: a sonda `replicacao_testar` (`servidor.rs`) monta uma
/// `Origem` com o que veio no pedido e cai no MESMO `replica::ligar` do laco.
/// Padrao que mora so no analisador do arquivo deixa esse irmao falando claro,
/// calado -- foi o que o pedido 373 pagou no ODBC, onde o `SQLConnect` monta a
/// receita sem passar pelo analisador.
pub const CIFRA_DE_SAIDA_PADRAO: bool = true;

/// O que a leitura dos interruptores de saida junta pelo caminho, e que o
/// [`Config`] pronto nao teria mais como saber.
///
/// Uma saida com `cifra` ligada nao diz se ALGUEM a escolheu: tanto pode ser o
/// `"cifra": true` escrito no arquivo quanto o padrao de fabrica. A diferenca
/// e a unica coisa que separa um aviso util de um aviso perpetuo -- quem
/// escreveu a decisao (qualquer uma das duas) nao precisa ouvi-la todo
/// arranque; quem HERDOU a virada precisa, porque e a conexao dele que pode
/// parar contra um servidor anterior ao aperto.
#[derive(Default)]
struct LeituraDasSaidas {
    /// As saidas que ficaram com o padrao de fabrica, uma a uma.
    de_fabrica: Vec<String>,
    /// Avisos de `cifra` com valor que nao e `true` nem `false`.
    tortos: Vec<String>,
}

impl LeituraDasSaidas {
    /// O `cifra` de uma saida, distinguindo os TRES estados que o
    /// `booleano_ou` funde em dois: ausente, escrito e TORTO.
    ///
    /// O torto e o que muda de natureza quando o padrao vira. Com o padrao
    /// desligado, "valor que nao entendi vira `false`" era inofensivo; com ele
    /// ligado seria um REBAIXAMENTO silencioso, que e a armadilha que o pedido
    /// 373 pagou no ODBC. Aqui o torto cai no padrao (cifrado) e AVISA: o
    /// servidor nao adivinha o que quem digitou `"sim"` queria, e tambem nao
    /// desliga a cifra por causa de um engano de digitacao.
    fn cifra(&mut self, o: &Json, rotulo: &str) -> bool {
        match o.campo("cifra") {
            None => self.sem_decisao_escrita(rotulo),
            Some(Json::Bool(b)) => *b,
            Some(_) => {
                self.tortos.push(format!(
                    "o campo \"cifra\" de {rotulo} nao e true nem false: li \
                     como CIFRADO, que e o padrao desde 18/09/2026. Escreva \
                     \"cifra\": true ou \"cifra\": false -- valor que o \
                     servidor nao entende nunca desliga a cifra sozinho."
                ));
                CIFRA_DE_SAIDA_PADRAO
            }
        }
    }

    /// A saida que nasceu no padrao porque ninguem escreveu nada -- inclusive
    /// a que nao tem ONDE escrever, que e o texto solto de `web.servidores`.
    fn sem_decisao_escrita(&mut self, rotulo: &str) -> bool {
        self.de_fabrica.push(rotulo.to_string());
        CIFRA_DE_SAIDA_PADRAO
    }
}

/// De onde a replica puxa os eventos.
#[derive(Clone)]
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
    /// Nasce LIGADO ([`CIFRA_DE_SAIDA_PADRAO`], 18/09/2026), e `"cifra": false`
    /// e o escape ESCRITO. O que ele custa e o que sempre custou, so que agora
    /// no sentido contrario: o SOURCE tem de atender o aperto, e um source de
    /// versao anterior nao atende -- quem replica de um deles escreve o escape
    /// nesta origem.
    pub cifra: bool,
    /// A chave publica que se ESPERA do source, em hexadecimal -- o pino.
    ///
    /// Vazia com `cifra` ligada e tunel SEM pino: protege da escuta passiva e
    /// nao protege de quem esta no meio, porque o atacante apresenta a chave
    /// dele e nao ha com o que comparar. O arranque avisa exatamente isso.
    pub chave_do_fio: String,
}

/// `Debug` a mao, pelo mesmo motivo do da [`Cifra`]: o derivado imprimiria o
/// token do source, o hash da senha e a senha em claro do caminho antigo. A
/// `Replicacao` guarda as origens num `Vec`, entao um `dbg!` despejaria as de
/// TODAS as origens de uma vez.
///
/// `chave_do_fio` fica visivel: e a chave PUBLICA do pino, e esconde-la so
/// atrapalharia o diagnostico de pino torto.
impl std::fmt::Debug for Origem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Origem {
            nome,
            host,
            porta,
            token: _,
            databases,
            reconectar_em,
            usuario,
            senha_hash: _,
            senha: _,
            cada_minutos,
            hora,
            cifra,
            chave_do_fio,
        } = self;
        f.debug_struct("Origem")
            .field("nome", nome)
            .field("host", host)
            .field("porta", porta)
            .field("token", &"(oculto)")
            .field("databases", databases)
            .field("reconectar_em", reconectar_em)
            .field("usuario", usuario)
            .field("senha_hash", &"(oculto)")
            .field("senha", &"(oculta)")
            .field("cada_minutos", cada_minutos)
            .field("hora", hora)
            .field("cifra", cifra)
            .field("chave_do_fio", chave_do_fio)
            .finish()
    }
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
    /// A chave publica que se ESPERA deste no quando `cluster.cifra` esta
    /// ligada -- o pino, no estilo `known_hosts` do SSH. Em hexadecimal.
    ///
    /// Cada no da lista descreve UM servidor "como os outros o alcancam", e o
    /// pino mora aqui pelo mesmo motivo que o `chave_do_fio` da origem: e a
    /// chave DESTE no, e quem pulsa ou replica dele a confere. Vazia com a
    /// cifra ligada = tunel SEM pino, que protege da escuta passiva e nao de
    /// quem esta no meio -- o arranque avisa exatamente isso.
    ///
    /// Faz parte da igualdade de proposito: girar o pino de um no muda a
    /// entrada, e o supervisor do pulso reconecta com o pino novo, como ja faz
    /// quando um no muda de endereco.
    pub chave_do_fio: String,
}

impl NoCluster {
    /// `host:porta`, do jeito que um cliente redirecionado usa.
    pub fn alvo(&self) -> String {
        format!("{}:{}", self.endereco, self.porta)
    }

    /// O pino deste no, ja em bytes -- ou o erro que diz o que corrigir.
    ///
    /// `None` = "sem pino", nunca "qualquer chave serve por engano": um pino
    /// escrito errado vira ERRO em vez de virar `None`, senao ele viraria em
    /// silencio um tunel sem pino -- o estrago que o pino existe para impedir.
    /// E a MESMA regra do [`Origem::pino_do_fio`].
    pub fn pino_do_fio(&self) -> Result<Option<[u8; 32]>> {
        if self.chave_do_fio.is_empty() {
            return Ok(None);
        }
        Ok(Some(chave_de_hex(
            &self.chave_do_fio,
            &format!("cluster.nos[{}].chave_do_fio", self.id),
        )?))
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
#[derive(Clone)]
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
    /// Quantos servidores tem de confirmar uma gravacao antes de o cliente
    /// ouvir "gravei" -- pedido 207, a transacao com quorum.
    ///
    /// # Ele e GUARDADO e ainda NAO e imposto, e isto esta escrito de proposito
    ///
    /// A escrita com quorum nao existe: hoje o master confirma sem esperar
    /// replica nenhuma (o cabecalho do `cluster.rs` diz isso sem eufemismo).
    /// O campo mora aqui desde antes por uma razao de formato -- **mudanca de
    /// formato entra cedo**, e o lugar dele e o bloco `cluster`, ao lado de
    /// `nos`, que e o que o 207 decidiu. Guardar agora custa um inteiro;
    /// descobrir depois que ele devia morar noutro bloco custa migracao.
    ///
    /// Zero = como hoje, o master confirma sozinho. A tela mostra o valor e
    /// diz, com todas as letras, que ele ainda nao e imposto -- campo que
    /// finge efeito e pior que campo ausente, e esta casa ja pagou por isso
    /// com o `recursos.cache_paginas`, que passou tres versoes no arquivo,
    /// no MANUAL e na tela sem uma linha de codigo o lendo.
    /// DIVIDA: #207 o quorum e guardado e nao imposto -- a escrita com quorum nao existe, e o master confirma sem esperar replica nenhuma
    pub quorum_minimo: u64,
    /// Credenciais com que ESTE no fala com os outros -- as mesmas tres
    /// pecas da origem de replicacao, e pela mesma razao: a senha nunca
    /// aparece em claro, so o hash de onde sai a chave do desafio-resposta.
    pub token: String,
    pub usuario: String,
    pub senha_hash: String,
    /// Cifrar TODO o trafego do cluster -- o pulso da eleicao E a replicacao
    /// entre os nos -- reaproveitando o aperto de mao do fio, com pino por no
    /// (`nos[].chave_do_fio`).
    ///
    /// Nasce LIGADO ([`CIFRA_DE_SAIDA_PADRAO`], 18/09/2026), e `"cifra": false`
    /// e o escape ESCRITO -- um cluster que fala em claro passa a dizer isso no
    /// arquivo. Vale para os DOIS caminhos de uma vez, de proposito: cifrar so
    /// o pulso ou so a replicacao deixaria metade do trafego protegida e a
    /// outra nao, que e pior que nenhuma, porque parece protegido.
    ///
    /// O que ele exige e do cluster INTEIRO: todo no tem de atender o aperto
    /// (`cifra_fio.ligada`, que ja nasce ligada). Um cluster com um no de
    /// versao anterior ao aperto escreve o escape ate atualizar o no.
    pub cifra: bool,
    /// Recusar TODO pulso que nao prove a identidade de quem o manda --
    /// pedido 278.
    ///
    /// # Pedida, nao imposta -- e o padrao e o do dia de ontem
    ///
    /// Nasce DESLIGADO de proposito. A prova do pulso (`pulso.rs`) sai
    /// sozinha assim que os dois lados tem `chave_do_fio`, e quem a recebe a
    /// confere sempre; o que este interruptor muda e o que acontece com o
    /// pulso que chega SEM prova nenhuma. Com ele ligado de fabrica, todo no
    /// de versao anterior pararia de ser ouvido de um dia para o outro --
    /// «proteção que quebra todo cliente antigo nao e protecao, e estrago».
    ///
    /// Quem nao o liga nao fica sem nada: o crivo se auto-eleva por par
    /// (`EstadoCluster::marcar_provado`), e um no que JA provou uma vez nao
    /// volta a ser aceito sem prova nesta vida do processo. O que o
    /// interruptor fecha e a janela do arranque, em que ninguem provou ainda
    /// -- e o caminho e o mesmo da cifra: sobem-se todos os nos, e so entao
    /// se liga.
    pub exigir_prova_do_pulso: bool,
}

/// `Debug` a mao: as credenciais com que ESTE no fala com os outros. O token
/// do cluster e portao 1 -- quem o tem alcanca o pulso da eleicao.
impl std::fmt::Debug for Cluster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Cluster {
            nos,
            id,
            prioridade,
            janela_s,
            pulso_s,
            avisar_cada_min,
            email,
            databases,
            quorum_minimo,
            token: _,
            usuario,
            senha_hash: _,
            cifra,
            exigir_prova_do_pulso,
        } = self;
        f.debug_struct("Cluster")
            .field("nos", nos)
            .field("id", id)
            .field("prioridade", prioridade)
            .field("janela_s", janela_s)
            .field("pulso_s", pulso_s)
            .field("avisar_cada_min", avisar_cada_min)
            .field("email", email)
            .field("databases", databases)
            .field("quorum_minimo", quorum_minimo)
            .field("token", &"(oculto)")
            .field("usuario", usuario)
            .field("senha_hash", &"(oculto)")
            .field("cifra", cifra)
            .field("exigir_prova_do_pulso", exigir_prova_do_pulso)
            .finish()
    }
}

impl Cluster {
    fn de_json(
        j: &Json,
        id_servidor: &str,
        saidas: &mut LeituraDasSaidas,
    ) -> Result<Option<Cluster>> {
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
                        chave_do_fio: n.texto_ou("chave_do_fio", "").trim().to_string(),
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
        // Lido ANTES do literal porque o rotulo conta os nos, e la dentro a
        // lista ja foi movida para o campo.
        let cifra = saidas.cifra(c, &format!("cluster ({} nos)", nos.len()));
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
            quorum_minimo: c.inteiro_ou("quorum_minimo", 0).max(0) as u64,
            token: c.texto_ou("token", "").to_string(),
            usuario: c.texto_ou("usuario", "").trim().to_string(),
            senha_hash: c.texto_ou("senha_hash", "").trim().to_string(),
            cifra,
            exigir_prova_do_pulso: c.booleano_ou("exigir_prova_do_pulso", false),
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
        // Pino torto e recusado na DECLARACAO, e nao no primeiro pulso: uma
        // chave escrita errada tem de derrubar o arranque com o no nomeado, em
        // vez de virar um tunel sem pino que ninguem pediu -- a mesma decisao
        // do `pino_torto_na_origem_e_erro_e_nao_ausencia`. A conferencia vale
        // ate sem `cifra` ligada: um pino guardado para ligar depois nao pode
        // estar torto esperando o dia em que alguem ligue.
        for n in &self.nos {
            n.pino_do_fio()?;
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
            ("quorum_minimo", Json::de_u64(self.quorum_minimo)),
            // O que o campo acima NAO faz, dito pelo servidor e nao pela tela:
            // duas telas divergem no dia em que uma for atualizada e a outra
            // nao, e a que envelhece e sempre a que ninguem compila.
            ("quorum_imposto", Json::Bool(false)),
            // O estado da cifra do cluster e um BOOLEANO informativo, como o
            // `email` acima -- o pino de cada no NAO sai daqui: a resposta de
            // protocolo nunca carrega o pino, e "cifra_do_no" por no diria
            // quem tem pino e quem nao, que e mapa para o atacante.
            ("cifra", Json::Bool(self.cifra)),
            // Booleano informativo pelo mesmo motivo do `cifra`: dizer QUAL no
            // ja provou identidade seria entregar, a quem pergunta, o mapa de
            // quem ainda esta sem prova -- que e exatamente por onde o pulso
            // forjado do pedido 278 entrava.
            (
                "exigir_prova_do_pulso",
                Json::Bool(self.exigir_prova_do_pulso),
            ),
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
                                // So o FATO de haver pino, nunca o pino: a tela
                                // precisa saber se o no vai cifrado com ou sem
                                // ancora, e o pino em si e config, nao resposta.
                                ("tem_pino", Json::Bool(!n.chave_do_fio.is_empty())),
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
    /// A saude do disco onde o banco grava (pedido 249): a sonda canario e
    /// o aviso imediato do erro de E/S. Independe de `ligado`, que e o vigia
    /// de ESPACO -- sao dois relogios com duas perguntas.
    pub disco: Disco,
    /// O canal de SMS, pelo gateway e-mail-para-SMS da operadora.
    pub sms: Sms,
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
            disco: Disco::default(),
            sms: Sms::default(),
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
            disco: Disco::de_json(a),
            sms: Sms::de_json(a)?,
        };
        // O SMS sai pelo MESMO rele do e-mail (e-mail-para-SMS da operadora):
        // ligado sem rele e um canal que promete e nunca entrega. A recusa vem
        // no arranque, e nao na primeira falha de disco.
        if alertas.sms.ligado && !alertas.email.ligado {
            return Err(PhxError::Esquema(
                "alertas.sms ligado sem alertas.email ligado: o SMS sai pelo rele \
                 de e-mail (numero@gateway), entao o e-mail precisa estar ligado"
                    .into(),
            ));
        }
        if alertas.sms.ligado {
            alertas.email.validar()?;
        }
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
        if alertas.email.ligado
            && (alertas.ligado || alertas.email.avisar_jobs || alertas.email.avisar_seguranca)
        {
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
            ("disco", self.disco.para_json()),
            ("sms", self.sms.para_json()),
        ])
    }
}

/// A sonda de saude do disco onde o banco grava (pedido 249).
///
/// # Por que nasce LIGADA, ao contrario do vigia de espaco
///
/// O vigia de espaco manda e-mail, e por isso nasce desligado -- aviso que
/// ninguem pediu e caixa de entrada cheia. A sonda so MEDE: escreve, sincroniza,
/// rele e apaga um arquivo de 64 bytes no `base`, uma vez por minuto, e mostra
/// o resultado no painel. Sem ela o cartao diria «nao medido» para todo mundo
/// que nunca abriu o config.json, e um monitor que nasce cego nao monitora
/// nada. O aviso por e-mail e SMS continua dependendo de `alertas.email` e
/// `alertas.sms` estarem ligados -- a sonda ligada sem rele so pinta o painel.
/// E a mesma decisao da telemetria, que nasce coletando.
///
/// Quem nao quer a escrita periodica escreve `"disco": {"ligado": false}`.
#[derive(Debug, Clone)]
pub struct Disco {
    pub ligado: bool,
    /// De quantos em quantos segundos a sonda escreve o canario.
    pub checar_segundos: u64,
    /// Silencio entre dois avisos do MESMO tipo de evento (E/S, so-leitura,
    /// sem espaco, conferencia). O primeiro e sempre imediato. Em minutos, e
    /// nao nas horas do vigia de espaco: disco cheio continua cheio por
    /// horas, erro de E/S e noticia aguda que merece ser repetida antes.
    pub repetir_minutos: u64,
    /// Acima disto a sonda que PASSOU vira estado `aviso` no painel (sem
    /// e-mail): disco que morre costuma ficar lento antes de falhar. Zero
    /// desliga o aviso de lentidao.
    pub lento_ms: u64,
}

impl Default for Disco {
    fn default() -> Self {
        Disco {
            ligado: true,
            checar_segundos: 60,
            repetir_minutos: 30,
            lento_ms: 1_000,
        }
    }
}

impl Disco {
    fn de_json(alertas: &Json) -> Disco {
        let padrao = Disco::default();
        let Some(d) = alertas.campo("disco") else {
            return padrao;
        };
        Disco {
            ligado: d.booleano_ou("ligado", padrao.ligado),
            checar_segundos: d
                .inteiro_ou("checar_segundos", padrao.checar_segundos as i64)
                .max(1) as u64,
            repetir_minutos: d
                .inteiro_ou("repetir_minutos", padrao.repetir_minutos as i64)
                .max(0) as u64,
            lento_ms: d.inteiro_ou("lento_ms", padrao.lento_ms as i64).max(0) as u64,
        }
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligado", Json::Bool(self.ligado)),
            ("checar_segundos", Json::de_u64(self.checar_segundos)),
            ("repetir_minutos", Json::de_u64(self.repetir_minutos)),
            ("lento_ms", Json::de_u64(self.lento_ms)),
        ])
    }
}

/// O canal de SMS: e-mail-para-SMS da operadora.
///
/// # Por que este meio, e nao um gateway HTTP
///
/// Todo gateway de SMS que se contrata hoje fala HTTPS, HTTPS pede TLS, e TLS
/// pede crate -- e zero dependencias e petrea. O que funciona sem crate
/// nenhuma e o que ja existe na casa: o rele de e-mail. Toda operadora que
/// oferece e-mail-para-SMS entrega `numero@gateway` como texto no celular,
/// e o `email::enviar` ja sabe falar com o rele. O meio definitivo e decisao
/// do dono, e esta escrita em `docs/SAUDE-DO-DISCO.md`.
///
/// O texto vai numa linha so, com ate 160 caracteres, e NUNCA carrega o
/// caminho do disco nem segredo: SMS atravessa a operadora em claro.
#[derive(Debug, Clone, Default)]
pub struct Sms {
    pub ligado: bool,
    /// Os numeros, so digitos e um `+` opcional na frente: eles viram a parte
    /// local de um endereco de e-mail.
    pub numeros: Vec<String>,
    /// O dominio do gateway da operadora (`sms.operadora.com.br`). Sem arroba:
    /// o endereco final e `numero@gateway`.
    pub gateway_email: String,
}

impl Sms {
    fn de_json(alertas: &Json) -> Result<Sms> {
        let Some(s) = alertas.campo("sms") else {
            return Ok(Sms::default());
        };
        let sms = Sms {
            ligado: s.booleano_ou("ligado", false),
            numeros: s
                .textos("numeros")
                .into_iter()
                .map(|n| n.trim().to_string())
                .collect(),
            gateway_email: s.texto_ou("gateway_email", "").trim().to_string(),
        };
        if sms.ligado {
            sms.validar()?;
        }
        Ok(sms)
    }

    fn validar(&self) -> Result<()> {
        if self.numeros.is_empty() {
            return Err(PhxError::Esquema(
                "alertas.sms ligado sem \"numeros\": nao ha para quem mandar".into(),
            ));
        }
        for n in &self.numeros {
            let digitos = n.strip_prefix('+').unwrap_or(n);
            if digitos.is_empty() || !digitos.chars().all(|c| c.is_ascii_digit()) {
                return Err(PhxError::Esquema(format!(
                    "alertas.sms.numeros: {n:?} nao e um numero (so digitos, com + opcional)"
                )));
            }
        }
        let g = &self.gateway_email;
        if g.is_empty() {
            return Err(PhxError::Esquema(
                "alertas.sms ligado sem \"gateway_email\": o dominio do gateway da operadora"
                    .into(),
            ));
        }
        // O gateway vira a metade direita de um endereco: arroba, espaco ou
        // quebra de linha ali seria um segundo endereco -- ou um cabecalho
        // injetado na mensagem.
        if g.contains(['@', ' ', '\r', '\n']) || !g.contains('.') {
            return Err(PhxError::Esquema(format!(
                "alertas.sms.gateway_email: {g:?} nao e um dominio (sem arroba, com ponto)"
            )));
        }
        Ok(())
    }

    /// Os enderecos `numero@gateway`, prontos para o `RCPT TO`.
    pub fn enderecos(&self) -> Vec<String> {
        self.numeros
            .iter()
            .map(|n| format!("{n}@{}", self.gateway_email))
            .collect()
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligado", Json::Bool(self.ligado)),
            (
                "numeros",
                Json::Lista(self.numeros.iter().map(Json::texto_de).collect()),
            ),
            ("gateway_email", Json::texto_de(&self.gateway_email)),
        ])
    }
}

// ---------------------------------------------------------------------------
// Os segredos que este servidor APRESENTA -- do arquivo ou do ambiente
// ---------------------------------------------------------------------------

/// Um segredo que este servidor precisa APRESENTAR a alguem -- a senha do rele
/// de e-mail, a do cofre, a privada do fio, a senha e o token de uma ligacao do
/// DbLink --, e que por isso se guarda inteiro, e nao em hash.
///
/// # Por que um tipo, e nao cinco `unwrap_or_default`
///
/// Os cinco donos liam `<campo>_env` do mesmo jeito, e erravam do mesmo jeito
/// (pedido 372): variavel DECLARADA e AUSENTE virava segredo VAZIO, calado.
/// Quem escreve `senha_env` esta tirando o segredo do arquivo; um nome digitado
/// errado, ou a variavel que o servico nao exporta, fazia o servidor apresentar
/// senha vazia -- e o erro vinha do OUTRO lado («access denied»), mandando
/// procurar a credencial no lugar errado. Contra um destino cujo usuario tem
/// senha vazia, autenticava.
///
/// «De onde vem este segredo» era UMA pergunta escrita cinco vezes; aqui ela e
/// escrita uma, em [`Segredo::ler`]. E o valor so sai por [`Segredo::valor`],
/// que devolve o ERRO NOMEADO quando a variavel faltou: nao ha caminho de
/// leitura que entregue o vazio no lugar do erro.
///
/// # Por que a falta NAO e erro na leitura
///
/// Porque quem le e o arranque inteiro. O `dblink.json` passa pelo
/// `Registro::abrir` com `?` antes de a porta abrir, e o `config.json` passa
/// pelo [`Config::ler`], que o `--usuarios` e o `--chave-do-fio` tambem chamam
/// -- de um terminal que nao tem o ambiente do servico. Recusar na leitura
/// faria uma variavel do DbLink derrubar o motor de dados, e um administrador
/// sem a variavel no terminal perder a linha de comando. A falta fica guardada,
/// sai como AVISO no arranque, e vira ERRO para quem precisa do valor -- e so
/// para ele.
#[derive(Clone, Default)]
pub struct Segredo {
    valor: String,
    origem: OrigemDoSegredo,
}

#[derive(Clone, Default)]
enum OrigemDoSegredo {
    #[default]
    Arquivo,
    Ambiente,
    /// A mensagem inteira, montada na leitura: e so ali que se sabe quem
    /// declarou a variavel.
    Ausente(String),
    /// Veio SELADO do arquivo e abriu com a chave mestra (pedido 372, o
    /// `dblink.json` cifrado): o valor esta aqui, e o disco nunca o ve em
    /// claro. Nao e `Arquivo` porque o aviso do texto puro fala com o
    /// `Arquivo`, e ele mentiria sobre um segredo que o disco guarda cifrado.
    Selado,
    /// Veio selado e NAO abriu -- sem chave, chave errada, envelope movido.
    ///
    /// `envelope` e o texto do arquivo, opaco: e ELE que volta ao disco na
    /// proxima gravacao. Sem a chave nao ha como selar de novo, e escrever
    /// outra coisa no lugar perderia a credencial de uma ligacao que so
    /// estava esperando a chave voltar.
    Trancado {
        motivo: String,
        envelope: String,
    },
}

/// `Debug` a mao, e pelo mesmo rotulo da tela: o derivado imprimiria o valor.
/// Nenhum dono o chama hoje -- os cinco escrevem o proprio `Debug` --, mas o
/// dia em que alguem trocar um `senha: _` por `.field("senha", &self.senha)`
/// nao pode ser o dia em que a senha vaza.
impl std::fmt::Debug for Segredo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.rotulo("(vazio)", "(oculto)", "(do ambiente)"))
    }
}

impl Segredo {
    /// Le o par `campo` / `campo_env` de um objeto do arquivo -- a UNICA funcao
    /// que decide de onde o segredo vem.
    ///
    /// Devolve tambem o nome da variavel (aparado; vazio = veio do arquivo),
    /// que cada dono guarda no seu `*_env` para a tela e para o disco.
    ///
    /// `quem` nomeia quem declarou, para a mensagem da falta: `alertas.email`,
    /// `a ligacao "erp" do DbLink`. E o que faz o erro apontar para a
    /// variavel, e nao para a autenticacao do outro lado.
    pub fn ler(o: &Json, campo: &str, quem: &str) -> (Segredo, String) {
        let campo_env = format!("{campo}_env");
        let var = o.texto_ou(&campo_env, "").trim().to_string();
        if var.is_empty() {
            let valor = o.texto_ou(campo, "").to_string();
            return (
                Segredo {
                    valor,
                    origem: OrigemDoSegredo::Arquivo,
                },
                var,
            );
        }
        let segredo = match std::env::var(&var) {
            Ok(valor) => Segredo {
                valor,
                origem: OrigemDoSegredo::Ambiente,
            },
            Err(e) => {
                let motivo = match e {
                    std::env::VarError::NotPresent => "nao existe",
                    std::env::VarError::NotUnicode(_) => "existe, mas nao e texto UTF-8,",
                };
                // O nome da variavel NAO e segredo -- a tela e o `Debug` ja o
                // mostram --, e e ele que diz onde procurar. O valor, este
                // caminho nem tem.
                Segredo {
                    valor: String::new(),
                    origem: OrigemDoSegredo::Ausente(format!(
                        "{quem} declara {campo_env} = {var:?}, e essa variavel \
                         {motivo} no ambiente deste processo. O segredo NAO \
                         virou vazio: corrija o nome, ou exporte a variavel no \
                         ambiente de quem sobe o servidor e reinicie"
                    )),
                }
            }
        };
        (segredo, var)
    }

    /// Um segredo que veio SELADO do arquivo e abriu (pedido 372).
    ///
    /// O dono que abre o envelope e quem constroi: este tipo nao sabe de sal,
    /// de nonce nem de AAD -- continua sabendo so de ONDE o valor veio.
    pub fn aberto_do_envelope(valor: String) -> Segredo {
        Segredo {
            valor,
            origem: OrigemDoSegredo::Selado,
        }
    }

    /// Um segredo que veio selado e NAO abriu. Ver [`OrigemDoSegredo::Trancado`].
    pub fn trancado(motivo: String, envelope: String) -> Segredo {
        Segredo {
            valor: String::new(),
            origem: OrigemDoSegredo::Trancado { motivo, envelope },
        }
    }

    /// O valor, para APRESENTAR -- ou o erro que nomeia a variavel que faltou,
    /// ou a chave que nao abriu o envelope.
    pub fn valor(&self) -> Result<&str> {
        match &self.origem {
            OrigemDoSegredo::Ausente(motivo) | OrigemDoSegredo::Trancado { motivo, .. } => {
                Err(PhxError::Esquema(motivo.clone()))
            }
            _ => Ok(&self.valor),
        }
    }

    /// Por que o envelope nao abriu -- `None` quando nao ha envelope trancado.
    pub fn trancado_por(&self) -> Option<&str> {
        match &self.origem {
            OrigemDoSegredo::Trancado { motivo, .. } => Some(motivo),
            _ => None,
        }
    }

    /// O envelope como estava no arquivo, para voltar IGUAL ao disco.
    pub fn envelope_trancado(&self) -> Option<&str> {
        match &self.origem {
            OrigemDoSegredo::Trancado { envelope, .. } => Some(envelope),
            _ => None,
        }
    }

    /// O segredo veio de um envelope aberto, e por isso nao pode voltar ao
    /// disco em claro.
    pub fn veio_selado(&self) -> bool {
        matches!(self.origem, OrigemDoSegredo::Selado)
    }

    /// A gravacao acabou de selar este segredo: ele deixa de estar escrito em
    /// claro. Devolve se mudou -- e a conta de «quantas sairam do texto puro»
    /// que a migracao DIZ, em vez de calar.
    pub fn passou_a_selado(&mut self) -> bool {
        if self.escrito_no_arquivo() {
            self.origem = OrigemDoSegredo::Selado;
            true
        } else {
            false
        }
    }

    /// A mensagem da falta, para o aviso do arranque. `None` quando nada
    /// faltou -- inclusive quando o segredo esta vazio no arquivo, que e
    /// decisao escrita, e nao variavel perdida.
    pub fn falta(&self) -> Option<&str> {
        match &self.origem {
            OrigemDoSegredo::Ausente(motivo) => Some(motivo),
            _ => None,
        }
    }

    /// O segredo esta ESCRITO no arquivo, em texto puro?
    ///
    /// Vazio nao conta: nao ha o que vazar. Vindo do ambiente tambem nao: o
    /// arquivo guarda so o nome da variavel.
    pub fn escrito_no_arquivo(&self) -> bool {
        matches!(self.origem, OrigemDoSegredo::Arquivo) && !self.valor.is_empty()
    }

    /// O rotulo que a tela e o protocolo mostram NO LUGAR do valor -- nunca o
    /// valor, nem mascarado com asteriscos do tamanho certo: o tamanho ja e
    /// informacao.
    ///
    /// Os tres rotulos vem do dono porque cada um ja tinha os seus (o token e
    /// «oculto», a senha e «oculta», a privada vazia e «do arquivo»), e trocar
    /// um rotulo e trocar o protocolo. O quarto estado e deste tipo, e e o
    /// mesmo para os cinco: `(variavel ausente)`.
    pub fn rotulo<'a>(&self, vazio: &'a str, oculto: &'a str, do_ambiente: &'a str) -> &'a str {
        match self.origem {
            OrigemDoSegredo::Ausente(_) => "(variavel ausente)",
            // O quinto estado, e o mesmo para todo dono, pela razao do de cima.
            OrigemDoSegredo::Trancado { .. } => "(cifra trancada)",
            _ if self.valor.is_empty() => vazio,
            OrigemDoSegredo::Arquivo | OrigemDoSegredo::Selado => oculto,
            OrigemDoSegredo::Ambiente => do_ambiente,
        }
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
#[derive(Clone, Default)]
pub struct Email {
    pub ligado: bool,
    /// Avisar tambem sobre JOBS: quando um falha, e quando um esta parado.
    ///
    /// Opt-in de proposito, e separado do `ligado`: quem configurou e-mail so
    /// para o disco apertado nao pode comecar a receber aviso de job por
    /// causa de uma versao nova. Guarda nova entra pedida, nao imposta.
    pub avisar_jobs: bool,
    /// Avisar tambem sobre VIOLACAO GRAVE: comando proibido pela politica,
    /// base proibida, travessia de diretorio -- o que faz o IP entrar na
    /// blacklist.
    ///
    /// Opt-in pelo mesmo motivo do `avisar_jobs`, e nao por simetria: quem
    /// configurou e-mail para o disco apertado nao pode comecar a receber
    /// aviso de seguranca por causa de uma versao nova. Guarda nova entra
    /// pedida, nao imposta.
    pub avisar_seguranca: bool,
    pub servidor: String,
    pub porta: u16,
    pub de: String,
    pub para: Vec<String>,
    pub usuario: String,
    /// PRIVADO de proposito: quem quiser ler passa por [`Email::senha`], e o
    /// `para_json` nunca a inclui. E a mesma regra da senha do usuario -- a
    /// diferenca e que esta o servidor precisa apresentar ao rele, entao nao
    /// da para guardar so o hash.
    senha: Segredo,
    pub assunto: String,
    pub timeout_s: u64,
}

/// `Debug` a mao: a senha do rele. O comentario do campo dizia «o `para_json`
/// nunca a inclui», e era verdade -- `Debug` e a OUTRA saida, e estava aberta.
impl std::fmt::Debug for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Email {
            ligado,
            avisar_jobs,
            avisar_seguranca,
            servidor,
            porta,
            de,
            para,
            usuario,
            senha: _,
            assunto,
            timeout_s,
        } = self;
        f.debug_struct("Email")
            .field("ligado", ligado)
            .field("avisar_jobs", avisar_jobs)
            .field("avisar_seguranca", avisar_seguranca)
            .field("servidor", servidor)
            .field("porta", porta)
            .field("de", de)
            .field("para", para)
            .field("usuario", usuario)
            .field("senha", &"(oculta)")
            .field("assunto", assunto)
            .field("timeout_s", timeout_s)
            .finish()
    }
}

impl Email {
    fn de_json(j: &Json) -> Result<Email> {
        let Some(e) = j.campo("email") else {
            return Ok(Email::default());
        };
        // A senha pode vir de variavel de ambiente. E o caminho recomendado:
        // config.json costuma ir para o controle de versao, e variavel de
        // ambiente nao. A variavel que falta NAO vira senha vazia -- ver
        // [`Segredo`].
        let (senha, _) = Segredo::ler(e, "senha", "alertas.email");
        Ok(Email {
            ligado: e.booleano_ou("ligado", false),
            avisar_jobs: e.booleano_ou("avisar_jobs", false),
            avisar_seguranca: e.booleano_ou("avisar_seguranca", false),
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
    ///
    /// `Err` quando `senha_env` nomeia variavel que este processo nao tem: o
    /// rele recebe o erro que aponta a variavel, e nunca uma senha vazia.
    pub fn senha(&self) -> Result<&str> {
        self.senha.valor()
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
            ("avisar_seguranca", Json::Bool(self.avisar_seguranca)),
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
            //
            // O rele nunca separou arquivo de ambiente na tela, e continua sem
            // separar: o rotulo e protocolo, e trocar «(oculta)» por «(do
            // ambiente)» aqui seria mudar o que a tela recebe sem ninguem ter
            // pedido. O que entra e o quarto estado, a variavel que faltou.
            (
                "senha",
                Json::texto_de(self.senha.rotulo("(vazia)", "(oculta)", "(oculta)")),
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
/// DIVIDA: ligar a cifra nao alcanca o diario que ja existe em claro -- um append-only nao se reescreve, e nao ha migracao que cifre para tras
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
    senha: Segredo,
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
    /// As tabelas que o dono DECLARA sigilosas, em nome qualificado
    /// `"banco.tabela"`.
    ///
    /// # Por que qualificado, e nao uma secao por banco
    ///
    /// Porque o nome de tabela nao e unico neste servidor: `loja.clientes` e
    /// `rh.clientes` sao duas tabelas, e uma lista de nomes soltos protegeria
    /// as duas ou nenhuma. Um objeto `{"loja": ["clientes"]}` diria o mesmo em
    /// dois niveis, e faria a lista de UMA tabela custar tres linhas; a forma
    /// plana e a mesma que `rest.tabelas` ja usa na tela.
    ///
    /// # O que esta lista faz HOJE, e o que ela nao faz
    ///
    /// Ela DECLARA. Nao cifra nada: cifrar uma tabela que ja existe e
    /// reescrever o `.reg` slot a slot, e isso e a migracao
    /// `Criptografar`/`Descriptografar`, que ainda nao existe -- pedido #268,
    /// com o desenho ja decidido em `docs/SEGURANCA.md` §13.6.
    /// O leitor que ela tem hoje e o **Profiler**, que para de gravar o texto
    /// do pedido no `perfil.txt` quando o pedido toca uma tabela desta lista.
    ///
    /// A fonte da verdade sobre "isto esta cifrado?" continua sendo o DISCO --
    /// o cabecalho do `.reg` diz, e e ele que a leitura consulta. A lista e a
    /// INTENCAO; o disco e o estado. Ver `docs/SEGURANCA.md` §13.
    /// DIVIDA: #268 marcar a tabela nao cifra o que ja esta gravado -- a migracao que reescreve o `.reg` slot a slot nao existe, entao a lista declara mais do que o disco cumpre
    pub tabelas: Vec<String>,
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
            .field("tabelas", &self.tabelas)
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
        //
        // Este e o QUINTO dono, e nao estava na lista do pedido 372: o
        // parecer contou quatro. Responde a mesma pergunta pelo mesmo
        // caminho, entao e irmao -- e aqui a falta nem era calada, era pior:
        // o cofre recusava dizendo «preencha cifra.senha ou cifra.senha_env»
        // a quem tinha preenchido `senha_env`.
        let (senha, senha_env) = Segredo::ler(c, "senha", "cifra");
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
            // Aparadas e em minusculas JA na leitura, e nao na comparacao:
            // quem compara aparando faz cada leitor repetir a regra, e o
            // leitor que esquecer erra calado -- que e exatamente o defeito
            // que uma lista de seguranca nao pode ter.
            tabelas: c
                .textos("tabelas")
                .into_iter()
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty())
                .collect(),
        }
    }

    /// A lista de tabelas sigilosas esta escrita de um jeito que os leitores
    /// entendem?
    ///
    /// Recusa no ARRANQUE, e nao na primeira gravacao. O motivo e o mesmo do
    /// `modo_e_ajuste`: uma lista de seguranca escrita errada nao da erro
    /// nenhum -- ela simplesmente nao casa com tabela nenhuma, e a protecao
    /// que o dono acha que ligou nunca acontece. Protecao que falha calada e
    /// pior que protecao ausente, porque ninguem vai procurar por ela.
    pub fn validar(&self) -> Result<()> {
        for t in &self.tabelas {
            let mut partes = t.split('.');
            let banco = partes.next().unwrap_or("");
            let tabela = partes.next().unwrap_or("");
            if banco.is_empty() || tabela.is_empty() || partes.next().is_some() {
                return Err(PhxError::Esquema(format!(
                    "cifra.tabelas: {t:?} nao e um nome qualificado -- escreva \
                     \"banco.tabela\" (o nome da tabela sozinho nao diz de qual \
                     banco, e este servidor pode ter duas com o mesmo nome)"
                )));
            }
        }
        let mut vistas: Vec<&str> = Vec::new();
        for t in &self.tabelas {
            if vistas.contains(&t.as_str()) {
                return Err(PhxError::Esquema(format!(
                    "cifra.tabelas: {t:?} aparece duas vezes"
                )));
            }
            vistas.push(t);
        }
        Ok(())
    }

    /// A senha do BANCO nao pode ser a senha de um ADMINISTRADOR.
    ///
    /// Palavra do dono, 05/09/2026: *«se a pessoa so vai acessar o banco via
    /// seu ERP usa a senha do banco. Se a pessoa vai administrar os bancos usa
    /// a senha de administracao que e diferente dos bancos. Porque se fosse a
    /// mesma, o invasor capturasse com sniffer ou wireshark nao conseguiria
    /// administrar e invadir o banco. Pois e uma senha diferente do banco. E
    /// nao trafegou na rede.»*
    ///
    /// # A assimetria que faz a regra valer, e ela e tecnica
    ///
    /// A senha da CONTA nao precisa viajar: o desafio-resposta manda uma
    /// `prova` derivada do hash, e o servidor guarda so o hash. A senha do
    /// BANCO **precisa** viajar, porque o servidor precisa dela em claro para
    /// derivar a chave do PBKDF2 -- nao ha desafio-resposta que a substitua.
    ///
    /// Entao quem escuta o fio pode, no pior caso, chegar na senha do banco. Se
    /// ela fosse tambem a de administracao, esse mesmo atacante viraria
    /// administrador. **Separar as duas transforma o pior caso de «tomou o
    /// servidor» em «leu um banco»** -- e a diferenca inteira esta em so uma
    /// delas viajar.
    ///
    /// # Por que comparar assim, e nao as senhas
    ///
    /// Porque a senha do administrador **nao existe em claro em lugar nenhum**
    /// -- ha so o hash. `senha::conferir` deriva a candidata com o SAL e as
    /// ITERACOES que o proprio hash carrega e compara em tempo constante. Isso
    /// responde «e a mesma senha?» sem este codigo nunca ter a do
    /// administrador, que e exatamente o ponto.
    ///
    /// # E por que a resposta e um `bool` seco
    ///
    /// Porque a recusa **nao pode virar oraculo**. Ela diz «a senha do banco
    /// nao pode ser a senha de administracao», e so: nunca QUAL administrador,
    /// nunca uma mensagem diferente conforme quem colidiu. Ela ja e um oraculo
    /// pequeno por natureza -- confirma que a senha tentada e a de ALGUM
    /// administrador --, e e por isso que a conferencia so acontece para quem
    /// **ja e administrador autenticado**, que e quem define senha de banco de
    /// qualquer jeito. Quem "melhorar" esta mensagem depois esta abrindo o
    /// oraculo; o motivo esta aqui e em `docs/SEGURANCA.md` §13.12.
    pub fn senha_e_de_algum_administrador(
        senha: &str,
        cadastro: &crate::usuarios::Cadastro,
    ) -> bool {
        if senha.is_empty() {
            return false;
        }
        cadastro
            .root
            .iter()
            .chain(cadastro.usuarios.iter())
            .filter(|u| u.supervisor)
            .any(|u| phxsql_core::senha::conferir(senha, &u.senha_hash))
    }

    /// Esta tabela foi declarada sigilosa?
    ///
    /// O `database` vazio nunca casa: um pedido que nao nomeia banco nao
    /// nomeia tabela desta lista, e casar so pelo nome da tabela protegeria
    /// `rh.clientes` porque alguem declarou `loja.clientes`.
    pub fn tabela_sigilosa(&self, database: &str, tabela: &str) -> bool {
        if database.is_empty() || tabela.is_empty() || self.tabelas.is_empty() {
            return false;
        }
        let alvo = format!(
            "{}.{}",
            database.trim().to_ascii_lowercase(),
            tabela.trim().to_ascii_lowercase()
        );
        self.tabelas.contains(&alvo)
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
    ///
    /// `Err` quando `senha_env` nomeia variavel que este processo nao tem.
    pub fn senha(&self) -> Result<&str> {
        self.senha.valor()
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
        // A falta da variavel RECUSA o arranque aqui, como a senha vazia ja
        // recusava -- e de proposito, ao contrario do DbLink: o cofre nao e
        // acessorio. Subir sem a chave deixaria o diario cifrado sem abrir, e
        // a unica coisa que muda e o erro, que passa a nomear a variavel.
        phxsql_store::cofre::definir_com(self.senha()?, self.iteracoes, modo, ajuste)
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
                Json::texto_de(self.senha.rotulo("(vazia)", "(oculta)", "(do ambiente)")),
            ),
            // A lista SAI inteira. Nome de tabela nao e segredo -- ele ja
            // aparece no esquema, no `.ndx` e na tela de estrutura --, e
            // esconde-la faria a tela de configuracao mostrar um campo que
            // ninguem consegue conferir.
            (
                "tabelas",
                Json::Lista(self.tabelas.iter().map(Json::texto_de).collect()),
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
/// pede. `exigir` e que carrega a decisao dificil -- e desde 18/09/2026 ela
/// nasce LIGADA tambem.
///
/// Cifra pedida e cifra que o atacante ativo apaga do pedido: ele corta o
/// `cifrar` do fio, o cliente rebaixa para claro, e a protecao vira zero.
/// Contra ele so vale `exigir: true` -- que quebra todo cliente velho, e por
/// isso e uma decisao de quem implanta, e nao um padrao herdado.
///
/// **Com `exigir` desligado, o tunel protege contra escuta PASSIVA e nada
/// mais.** Esta frase esta aqui, no `docs/SEGURANCA.md` e na tela pelo mesmo
/// motivo: e a que o leitor nao pode ter de adivinhar.
///
/// # O padrao que o dono mandou trocar, e as duas metades da troca (18/09/2026)
///
/// Ordem do dono: *"a comunicacao deve obrigatoriamente ser cifrada"*. A
/// ENTRADA virou primeiro: `exigir` nasce `true`, com `"exigir": false` como
/// escape ESCRITO -- o mesmo padrao da chave que nasce conferida. Duas coisas
/// tinham de cair antes, e cairam junto, porque separadas cada uma anunciaria
/// uma protecao que a vizinha nao presta:
///
/// 1. **O interruptor nao alcancava o que anunciava.** `exigir` decidia em UM
///    lugar -- o laco da porta de dados --, e no mesmo servidor e no mesmo
///    instante `POST /api {"op":"login"}` devolvia 200 com a sessao aberta e a
///    senha em claro; REST, MCP e o explorador idem. Hoje as tres portas HTTP
///    recusam pelo mesmo `portao_de_rede_http`, com `"atras_de_proxy": true`
///    como o escape ESCRITO delas, e o `encryption_exigida` parou de anunciar
///    o que o canal nao presta.
/// 2. **Os clientes desta casa nao podiam ser os quebrados.** O console e o
///    driver ODBC aprenderam o aperto (`--sem-cifra` e `CIFRA=0` sao os
///    escapes), e as bancadas escreveram o escape onde medem em claro de
///    proposito. Padrao novo que o proprio console do projeto nao alcanca e
///    meia funcionalidade.
///
/// A outra metade e a SAIDA, e ela veio depois com a mesma forma: `exigir` e
/// *inbound-only* -- decide sobre quem conecta NESTE servidor e nada sobre o
/// que este servidor CONECTA. Os tres interruptores de saida
/// (`replicacao.origens[].cifra`, `cluster.cifra`, `web.servidores[].cifra`)
/// nascem LIGADOS pelo [`CIFRA_DE_SAIDA_PADRAO`], com `"cifra": false` como
/// escape escrito de cada um. Numero e detalhe em `docs/SEGURANCA.md` 7.0.
#[derive(Clone)]
pub struct CifraFio {
    /// O servidor ATENDE o aperto. `false` recusa -- e a unica maneira de um
    /// servidor dizer "aqui nao tem".
    pub ligada: bool,
    /// Recusa qualquer pedido fora do tunel.
    pub exigir: bool,
    /// Recusa o login que NAO amarra a credencial ao canal, quando ha tunel.
    ///
    /// A amarracao (`amarrar_canal` no login) e PEDIDA pelo cliente -- e um
    /// atacante ativo que terminou o tunel do cliente corta o campo antes de
    /// reencaminhar, a mesma aritmetica do rebaixamento do `exigir`. Contra
    /// ele so vale o servidor EXIGIR a amarracao, decisao de quem implanta.
    /// So morde quando ha tunel: em claro nao ha transcricao a que amarrar.
    /// Nasce DESLIGADA -- guarda nova entra pedida, nao imposta.
    pub exigir_amarra: bool,
    /// PRIVADA de proposito: quem quiser ler passa por [`CifraFio::estatica`],
    /// e o `para_json` nunca a inclui.
    chave_privada: Segredo,
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
            .field("exigir_amarra", &self.exigir_amarra)
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
            // Ordem do dono, 18/09/2026: *"a comunicacao deve obrigatoriamente
            // ser cifrada"*. Nasce EXIGINDO, e `"exigir": false` e o escape
            // ESCRITO -- o mesmo padrao do `"verificar": false` da chave que
            // nasce conferida: escolha escrita em vez de omissao.
            //
            // A linha so pode estar assim porque o interruptor voltou a ser UM
            // (pedido 370): as portas HTTP recusam pelo `portao_de_rede_http`,
            // com `"atras_de_proxy": true` como o escape escrito DELAS, e o
            // `encryption_exigida` parou de anunciar o que nao presta.
            // Ligar isto antes teria transformado um furo conhecido em
            // garantia anunciada.
            exigir: true,
            exigir_amarra: false,
            chave_privada: Segredo::default(),
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
        let (chave_privada, chave_privada_env) = Segredo::ler(c, "chave_privada", "cifra_fio");
        CifraFio {
            ligada: c.booleano_ou("ligada", padrao.ligada),
            exigir: c.booleano_ou("exigir", padrao.exigir),
            exigir_amarra: c.booleano_ou("exigir_amarra", padrao.exigir_amarra),
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
    ///
    /// Fica SOB DEMANDA (e nao resolvido de uma vez dentro de `Config::ler`,
    /// como os outros seis campos de caminho) porque `estatica` decide se
    /// CRIA o arquivo -- e criar cedo demais, so por causa da leitura do
    /// config, faria um servidor com quem ninguem faz aperto escrever um
    /// arquivo que antes nao escrevia.
    pub fn caminho_da_chave(&self, config_em: Option<&Path>) -> PathBuf {
        resolver_caminho_do_config(&self.arquivo, config_em)
    }

    /// A privada estatica do servidor, e os avisos que a busca gerou.
    ///
    /// Ordem: variavel de ambiente, `config.json`, arquivo proprio. O arquivo
    /// e CRIADO na primeira vez -- e so na primeira vez que alguem de fato
    /// pede o aperto, para um servidor com quem ninguem faz aperto nao passar
    /// a escrever arquivo que antes nao escrevia.
    pub fn estatica(&self, config_em: Option<&Path>) -> Result<([u8; 32], Vec<String>)> {
        let mut avisos = Vec::new();
        // Variavel DECLARADA manda -- inclusive quando falta ou chega vazia.
        //
        // Aqui o vazio calado era o pior dos cinco donos do pedido 372: a
        // privada vazia fazia a busca seguir para o ARQUIVO, e o servidor
        // trocava de identidade sem uma linha dizendo por que -- lia uma
        // chave antiga, ou sorteava e gravava uma nova. O pino de todo
        // cliente quebrava, e o `--chave-do-fio` imprimia a publica ERRADA
        // para o operador pinar. Agora a falta e o erro que nomeia a variavel,
        // e a busca so segue para o arquivo quando ninguem declarou nada.
        let privada = self.chave_privada.valor()?;
        if !self.chave_privada_env.is_empty() || !privada.trim().is_empty() {
            let de_onde = if self.chave_privada_env.is_empty() {
                "cifra_fio.chave_privada".to_string()
            } else {
                format!("a variavel {}", self.chave_privada_env)
            };
            return Ok((chave_de_hex(privada, &de_onde)?, avisos));
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
            ("exigir_amarra", Json::Bool(self.exigir_amarra)),
            (
                "arquivo",
                Json::texto_de(self.arquivo.display().to_string()),
            ),
            ("chave_privada_env", Json::texto_de(&self.chave_privada_env)),
            // Nunca a privada -- nem mascarada, que o tamanho ja e informacao.
            (
                "chave_privada",
                Json::texto_de(self.chave_privada.rotulo(
                    "(do arquivo)",
                    "(oculta)",
                    "(do ambiente)",
                )),
            ),
        ])
    }
}

/// Le 32 bytes em hexadecimal, dizendo de onde vieram quando estao errados.
pub(crate) fn chave_de_hex(texto: &str, de_onde: &str) -> Result<[u8; 32]> {
    bytes32_de_hex(
        texto,
        &format!("a chave do fio em {de_onde}"),
        "e a X25519 tem 32",
    )
    .map_err(PhxError::Esquema)
}

/// 32 bytes em hexadecimal -- a UNICA leitura, para as duas chaves da casa que
/// chegam assim (a do fio e a mestra do DbLink). Devolve o motivo em texto, e
/// nao o erro pronto: a mestra do DbLink que nao se le nao e erro de quem
/// chamou, e fica guardada como motivo da tranca.
///
/// O motivo NUNCA carrega o texto lido -- so de onde veio e o tamanho.
fn bytes32_de_hex(texto: &str, quem: &str, precisa: &str) -> std::result::Result<[u8; 32], String> {
    let limpo: String = texto.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = phxsql_core::hash::de_hex(&limpo)
        .ok_or_else(|| format!("{quem} nao e hexadecimal valido"))?;
    if bytes.len() != 32 {
        return Err(format!("{quem} tem {} bytes, {precisa}", bytes.len()));
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(k)
}

// ---------------------------------------------------------------------------
// A chave mestra do DbLink (pedido 372)
// ---------------------------------------------------------------------------

/// Os quatro campos que declaram a chave mestra, e o que cada um entrega.
///
/// O TIPO mora no NOME do campo, e nao no formato do texto: adivinhar «64
/// caracteres hexadecimais e chave pronta» faria a senha que por acaso e
/// hexadecimal virar outra chave, calada -- e o cadastro cifrado com ela nao
/// abriria mais com a senha que o dono acha que usou.
const FONTES_DA_CHAVE_MESTRA: [(&str, bool); 4] = [
    // (campo, e senha -- `false` e a chave de 32 bytes ja derivada)
    ("senha_mestra_env", true),
    ("senha_mestra_arquivo", true),
    ("chave_mestra_env", false),
    ("chave_mestra_arquivo", false),
];

/// A chave mestra que cifra as credenciais do `dblink.json` -- decisao do
/// dono, 24/09/2026: cifra com chave mestra EXTERNA.
///
/// # De onde a chave vem, e de onde ela NAO pode vir
///
/// De FORA do conjunto que a copia carrega: uma variavel de ambiente, ou um
/// arquivo num caminho fora da pasta do `config.json`, da do `dblink.json` e
/// da dos dados. O modelo de ameaca e o do cofre (`cofre.rs`): o que se
/// protege e o arquivo COPIADO -- disco levado, backup vazado. Chave que viaja
/// na mesma copia protege contra ninguem e ANUNCIA protecao; o parecer do DBA
/// a recusou (`docs/propostas/parecer-dba-372-e-255.md` §1.5), e a recusa
/// acontece aqui, na DECLARACAO, e nao na primeira gravacao -- pela regra da
/// casa: recusar cedo custa um erro lido ao configurar, recusar tarde custa um
/// cadastro cifrado com a chave debaixo do capacho.
///
/// # Senha ou chave pronta
///
/// `senha_mestra_*` traz uma SENHA, que passa pelo PBKDF2 do material do
/// cadastro (290,3 ms medidos, uma vez por arranque). `chave_mestra_*` traz os
/// 32 bytes JA derivados, em hexadecimal: um HMAC com o sal (a subchave do
/// cadastro, ver `cofre::ChaveDeFora::Pronta`), e nenhum PBKDF2.
///
/// # Ausente nao derruba nada
///
/// A falta da variavel ou do arquivo e lida AQUI e guardada como motivo, pelo
/// mesmo contrato do [`Segredo`]: quem a sente e o cadastro, que tranca as
/// ligacoes cifradas e segue. O DbLink e acessorio; o motor nao e.
#[derive(Clone, Default)]
pub struct CifraDoDblink {
    /// O campo declarado, pelo nome -- vazio quando nenhum foi.
    pub campo: &'static str,
    /// O segredo de `*_env`, lido pelo [`Segredo::ler`] -- o unico leitor de
    /// segredo da casa. Vazio nas fontes de arquivo.
    segredo: Segredo,
    /// O nome da variavel de `*_env`.
    pub variavel: String,
    /// O arquivo de `*_arquivo`, resolvido ao lado do `config.json`.
    pub arquivo: PathBuf,
    /// Iteracoes do PBKDF2 do material NOVO. O que ja existe guarda as dele.
    pub iteracoes: u32,
    /// A declaracao torta, com o motivo: recusada pelo `Config::validar` e
    /// devolvida como indisponivel a quem pedir a chave mesmo assim.
    recusas: Vec<String>,
    /// O caminho REAL do arquivo da chave, como o `resolver` o conferiu -- e
    /// o unico que o [`CifraDoDblink::chave`] le. `None` num `Config` montado
    /// sem `ler`, que nao tem pastas contra as quais conferir.
    conferido: Option<PathBuf>,
}

/// `Debug` a mao pela regra da casa -- o [`Segredo`] ja se redige sozinho, mas
/// desestruturar sem `..` faz o campo novo parar de compilar aqui.
impl std::fmt::Debug for CifraDoDblink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let CifraDoDblink {
            campo,
            segredo,
            variavel,
            arquivo,
            iteracoes,
            recusas,
            conferido,
        } = self;
        f.debug_struct("CifraDoDblink")
            .field("campo", campo)
            .field("segredo", segredo)
            .field("variavel", variavel)
            .field("arquivo", arquivo)
            .field("iteracoes", iteracoes)
            .field("recusas", recusas)
            .field("conferido", conferido)
            .finish()
    }
}

/// A chave mestra do DbLink ja resolvida -- ou por que nao ha.
#[derive(Clone, Default)]
pub enum ChaveMestra {
    /// Ninguem declarou: o cadastro continua sendo o de sempre.
    #[default]
    NaoDeclarada,
    Senha(String),
    Pronta([u8; 32]),
    /// Declarada e indisponivel, com o motivo que nomeia o que falta.
    Indisponivel(String),
}

/// `Debug` a mao: as duas variantes do meio SAO a chave.
impl std::fmt::Debug for ChaveMestra {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChaveMestra::NaoDeclarada => f.write_str("NaoDeclarada"),
            ChaveMestra::Senha(_) => f.write_str("Senha(oculta)"),
            ChaveMestra::Pronta(_) => f.write_str("Pronta(oculta)"),
            ChaveMestra::Indisponivel(m) => f.debug_tuple("Indisponivel").field(m).finish(),
        }
    }
}

impl ChaveMestra {
    /// A chave no formato que o material do cofre entende, quando ha chave.
    pub fn de_fora(&self) -> Option<phxsql_store::cofre::ChaveDeFora<'_>> {
        match self {
            ChaveMestra::Senha(s) => Some(phxsql_store::cofre::ChaveDeFora::Senha(s)),
            ChaveMestra::Pronta(k) => Some(phxsql_store::cofre::ChaveDeFora::Pronta(k)),
            _ => None,
        }
    }
}

impl CifraDoDblink {
    fn de_json(j: &Json) -> CifraDoDblink {
        let mut c = CifraDoDblink {
            iteracoes: phxsql_store::cofre::ITERACOES_PADRAO,
            ..CifraDoDblink::default()
        };
        let Some(o) = j.campo("cifra_do_dblink") else {
            return c;
        };
        // Escrita torta RECUSA, como toda declaracao desta casa, e nao vira
        // «nao declarada»: quem escreveu `"cifra_do_dblink": "s3nh4"` acha que
        // cifrou, e o cadastro continuaria em claro (revisao SEC do 372, B3).
        // O valor torto nunca entra na mensagem -- ele pode ser a propria chave.
        if !matches!(o, Json::Objeto(_)) {
            c.recusas.push(
                "cifra_do_dblink tem de ser um objeto -- {\"chave_mestra_arquivo\": \
                 \"/caminho\"}, ou uma das outras tres fontes --, e o que esta \
                 escrito nao e (o valor nao se repete aqui: ele pode ser a chave)"
                    .to_string(),
            );
            return c;
        }
        for (campo, _) in FONTES_DA_CHAVE_MESTRA {
            if o.campo(campo).is_some_and(|v| v.texto().is_none()) {
                c.recusas.push(format!(
                    "cifra_do_dblink.{campo} tem de ser texto (o nome da variavel \
                     ou o caminho do arquivo)"
                ));
            }
        }
        if o.campo("iteracoes").is_some_and(|v| v.inteiro().is_none()) {
            c.recusas
                .push("cifra_do_dblink.iteracoes tem de ser um numero inteiro".to_string());
        }
        // O valor escrito no proprio `config.json` e a chave debaixo do
        // capacho: o `config.json` viaja na MESMA copia que o `dblink.json`.
        // O `Segredo::ler` o aceitaria, porque para os outros donos o arquivo
        // e um lugar legitimo -- aqui nao e, e a recusa vem antes dele.
        for escrito in ["senha_mestra", "chave_mestra"] {
            if o.campo(escrito).is_some() {
                c.recusas.push(format!(
                    "cifra_do_dblink.{escrito} escreve a chave mestra DENTRO do \
                     config.json, que viaja na mesma copia que o dblink.json: \
                     cifrar assim protege contra ninguem. Use {escrito}_env (uma \
                     variavel de ambiente) ou {escrito}_arquivo (um arquivo FORA \
                     da pasta do banco)"
                ));
            }
        }
        let declarados: Vec<(&'static str, bool)> = FONTES_DA_CHAVE_MESTRA
            .into_iter()
            .filter(|(campo, _)| !o.texto_ou(campo, "").trim().is_empty())
            .collect();
        if declarados.len() > 1 {
            let nomes: Vec<&str> = declarados.iter().map(|(n, _)| *n).collect();
            c.recusas.push(format!(
                "cifra_do_dblink declara {}: declare UMA fonte so -- com duas, o \
                 servidor teria de escolher calado qual chave vale",
                nomes.join(" e ")
            ));
        }
        if let Some((campo, _)) = declarados.first() {
            c.campo = campo;
            if let Some(base) = campo.strip_suffix("_env") {
                let (segredo, variavel) = Segredo::ler(o, base, "cifra_do_dblink");
                c.segredo = segredo;
                c.variavel = variavel;
            } else {
                c.arquivo = PathBuf::from(o.texto_ou(campo, "").trim());
            }
        }
        // A MESMA faixa que a abertura do cadastro confere no arquivo: a
        // declaracao nao pode pedir o que a leitura recusaria. O porque do piso
        // e do teto esta nas duas constantes.
        let (piso, teto) = (
            crate::dblink::ITERACOES_MINIMAS_DO_CADASTRO,
            crate::dblink::ITERACOES_MAXIMAS_DO_CADASTRO,
        );
        let iteracoes = o.inteiro_ou("iteracoes", phxsql_store::cofre::ITERACOES_PADRAO as i64);
        if !(piso as i64..=teto as i64).contains(&iteracoes) {
            c.recusas.push(format!(
                "cifra_do_dblink.iteracoes {iteracoes} fora da faixa ({piso} a {teto})"
            ));
        } else {
            c.iteracoes = iteracoes as u32;
        }
        c
    }

    /// A chave foi declarada? (Declarada nao quer dizer disponivel.)
    pub fn declarada(&self) -> bool {
        !self.campo.is_empty()
    }

    /// E senha (PBKDF2) ou chave pronta?
    fn e_senha(&self) -> bool {
        FONTES_DA_CHAVE_MESTRA
            .iter()
            .any(|(campo, senha)| *campo == self.campo && *senha)
    }

    /// Onde a chave foi declarada, para as mensagens: o campo e o que ele
    /// aponta -- nunca o valor.
    pub fn declarada_em(&self) -> String {
        if self.campo.is_empty() {
            return String::new();
        }
        if self.variavel.is_empty() {
            format!(
                "cifra_do_dblink.{} = {}",
                self.campo,
                self.arquivo.display()
            )
        } else {
            format!("cifra_do_dblink.{} = {}", self.campo, self.variavel)
        }
    }

    /// Resolve o arquivo ao lado do `config.json` e recusa o que cai DENTRO do
    /// conjunto copiado -- a pasta do `config.json`, a do `dblink.json` e a dos
    /// dados.
    ///
    /// Pelo caminho REAL, e nao pelo texto: `../banco/chave` e um link
    /// simbolico para dentro da pasta sao o mesmo capacho com outra grafia.
    /// O arquivo pode ainda nao existir (e ai ele esta so ausente, e nao
    /// recusado): resolve-se o ancestral mais longo que existe.
    fn resolver(&mut self, config_em: Option<&Path>, base: &Path, dblink: &Path) {
        if self.arquivo.as_os_str().is_empty() {
            return;
        }
        self.arquivo = resolver_caminho_do_config(&self.arquivo, config_em);
        let chave = caminho_real(&self.arquivo);
        self.conferido = Some(chave.clone());
        let pastas = [
            ("a do config.json", config_em.and_then(Path::parent)),
            ("a do dblink.json", dblink.parent()),
            ("a dos dados (\"base\")", Some(base)),
        ];
        for (qual, pasta) in pastas {
            let Some(pasta) = pasta else { continue };
            let pasta = if pasta.as_os_str().is_empty() {
                Path::new(".")
            } else {
                pasta
            };
            let real = caminho_real(pasta);
            if chave.starts_with(&real) {
                self.recusas.push(format!(
                    "cifra_do_dblink.{} aponta para {} (que o sistema resolve para \
                     {}), DENTRO da pasta que vai na copia do banco ({qual}, {}): o \
                     dblink.json cifrado e a chave que o abre viajariam juntos, e a \
                     cifra protegeria contra ninguem. Ponha a chave num caminho fora \
                     dessa pasta (um segredo montado, um diretorio proprio) ou use \
                     {}_env",
                    self.campo,
                    self.arquivo.display(),
                    chave.display(),
                    real.display(),
                    self.campo.trim_end_matches("_arquivo"),
                ));
                return;
            }
        }
    }

    /// A declaracao esta de pe? Recusa no ARRANQUE, com o motivo inteiro.
    pub fn validar(&self) -> Result<()> {
        if self.recusas.is_empty() {
            return Ok(());
        }
        Err(PhxError::Esquema(self.recusas.join("; ")))
    }

    /// A chave, lida AGORA -- ou por que nao ha.
    ///
    /// O arquivo e lido aqui, e nao na leitura do `config.json`: e o cadastro
    /// que precisa dela, e ele a pede uma vez por abertura.
    pub fn chave(&self) -> ChaveMestra {
        if !self.recusas.is_empty() {
            return ChaveMestra::Indisponivel(self.recusas.join("; "));
        }
        if self.campo.is_empty() {
            return ChaveMestra::NaoDeclarada;
        }
        let texto = if self.variavel.is_empty() {
            // Le O MESMO caminho que o `resolver` conferiu, e so se ele ainda
            // resolve para si mesmo: um diretorio trocado por um link DEPOIS
            // da conferencia levaria a leitura para dentro da pasta do banco
            // pelo caminho que ja tinha passado (revisao SEC do 372, M1).
            let alvo = match &self.conferido {
                Some(conferido) => {
                    let agora = caminho_real(conferido);
                    if agora != *conferido {
                        return ChaveMestra::Indisponivel(format!(
                            "cifra_do_dblink.{} foi conferido em {} e agora resolve \
                             para {}: o caminho mudou depois da conferencia (um \
                             diretorio trocado por link?), e a chave nao se le de \
                             onde nao foi conferida. Reinicie para conferir de novo",
                            self.campo,
                            conferido.display(),
                            agora.display()
                        ));
                    }
                    agora
                }
                None => caminho_real(&self.arquivo),
            };
            match std::fs::read_to_string(&alvo) {
                // Um `echo senha > arquivo` poe o fim de linha, e ele nao e
                // parte da senha. So ele sai: espaco no comeco ou no meio e
                // decisao de quem escreveu.
                Ok(t) => t.trim_end_matches(['\n', '\r']).to_string(),
                Err(e) => {
                    return ChaveMestra::Indisponivel(format!(
                        "cifra_do_dblink.{} aponta para {}, e esse arquivo nao abre \
                         neste processo ({e}). A chave NAO virou vazia: monte o \
                         arquivo e reinicie",
                        self.campo,
                        self.arquivo.display()
                    ))
                }
            }
        } else {
            if let Some(falta) = self.segredo.falta() {
                return ChaveMestra::Indisponivel(falta.to_string());
            }
            match self.segredo.valor() {
                Ok(v) => v.to_string(),
                Err(e) => return ChaveMestra::Indisponivel(e.corpo()),
            }
        };
        if texto.is_empty() {
            return ChaveMestra::Indisponivel(format!(
                "{} esta VAZIA -- chave vazia nao cifra nada, e fingir que cifra \
                 e pior que nao cifrar",
                self.declarada_em()
            ));
        }
        if self.e_senha() {
            return ChaveMestra::Senha(texto);
        }
        match bytes32_de_hex(
            &texto,
            &format!("a chave mestra de {}", self.declarada_em()),
            "e a chave mestra tem 32 (64 caracteres hexadecimais)",
        ) {
            Ok(k) => ChaveMestra::Pronta(k),
            Err(motivo) => ChaveMestra::Indisponivel(motivo),
        }
    }

    /// Para a tela e o protocolo: de onde a chave vem, NUNCA a chave.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("campo", Json::texto_de(self.campo)),
            ("variavel", Json::texto_de(&self.variavel)),
            (
                "arquivo",
                Json::texto_de(self.arquivo.display().to_string()),
            ),
            ("iteracoes", Json::de_u64(self.iteracoes as u64)),
        ])
    }
}

/// O caminho como o sistema operacional o ve: absoluto, com links simbolicos e
/// `..` resolvidos PELO SISTEMA, na ordem em que o kernel os resolve.
///
/// # O sistema primeiro, o texto so no fim
///
/// O `..` depois de um link sobe a partir do DESTINO do link, e nao do nome
/// dele: `fora/link/../chave.hex`, com `fora/link -> banco/sub`, e
/// `banco/chave.hex`. Resolver o `..` pelo texto antes de perguntar ao sistema
/// dava `fora/chave.hex`, passava pela conferencia, e o kernel abria a chave de
/// dentro da pasta do banco (revisao SEC do 372, M1, provada contra o sistema).
/// Entao o caminho INTEIRO vai ao `canonicalize` primeiro; se ele ainda nao
/// existe, vai o prefixo mais longo que existe, e so o sufixo que falta --
/// onde nao ha link nenhum a seguir -- e resolvido pelo texto.
fn caminho_real(p: &Path) -> PathBuf {
    use std::path::Component;
    let absoluto = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let partes: Vec<Component> = absoluto.components().collect();
    for corte in (0..=partes.len()).rev() {
        let prefixo: PathBuf = partes[..corte].iter().collect();
        let Ok(mut real) = std::fs::canonicalize(&prefixo) else {
            continue;
        };
        for parte in &partes[corte..] {
            match parte {
                Component::CurDir => {}
                Component::ParentDir => {
                    real.pop();
                }
                outra => real.push(outra.as_os_str()),
            }
        }
        return real;
    }
    absoluto
}

/// Abre `caminho` para escrita, NOVO e com permissao 0600 desde o primeiro
/// byte (no Unix; no Windows vale a ACL da pasta, como sempre valeu).
///
/// A permissao e posta na CRIACAO, e nao depois: entre criar aberto e apertar
/// ha uma janela em que qualquer um le o conteudo, e essa janela e a unica
/// coisa que um arquivo de segredo existe para nao ter. `create_new` e parte
/// da garantia: `mode` so vale para o arquivo que nasce aqui, e um que ja
/// existisse entraria com a permissao que tinha.
fn abrir_privado(caminho: &Path) -> std::io::Result<std::fs::File> {
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opcoes.mode(0o600);
    }
    opcoes.open(caminho)
}

/// Grava a estatica com permissao 0600 no Unix -- ver [`abrir_privado`].
fn gravar_chave(caminho: &Path, chave: &[u8; 32]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut arq = abrir_privado(caminho)?;
    writeln!(arq, "{}", phxsql_core::hash::para_hex(chave))?;
    arq.sync_all()
}

/// O temporario de [`gravar_privado`]: o nome INTEIRO mais `.tmp`.
///
/// Era `with_extension("tmp")`, que TROCA a extensao -- e com `--config
/// servidor.tmp` o temporario era o proprio config: a gravacao comecava
/// apagando-o (revisao SEC de 24/09/2026, pedido 450). E na troca de forma
/// era pior: o temporario do `servidor.phz` e o `servidor.tmp` que se estava
/// migrando. Acrescentar em vez de trocar da um nome sempre mais longo que o
/// do arquivo, entao nunca e ele; e nunca e o outro do par `.json`/`.phz`,
/// que termina em `.json` ou `.phz` e nao em `.tmp` acrescentado a um deles.
/// Uma funcao so, para os testes perguntarem o MESMO nome que se grava.
pub(crate) fn temporario_de(caminho: &Path) -> PathBuf {
    let mut nome = caminho
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    nome.push(".tmp");
    caminho.with_file_name(nome)
}

/// Grava `corpo` em `caminho` de forma atomica e 0600 desde o primeiro byte.
///
/// # Os tres irmaos que faziam o contrario
///
/// A licao do `create_new + mode(0o600)` e de 30/08/2026 (`d3b7d62`) e
/// entrou para a chave do fio -- e nao voltou aos irmaos: `config.json` (o
/// token e os hashes), `dblink.json` (senha e token do outro banco) e
/// `jobs.json` escreviam com `std::fs::write`, na permissao do `umask`, e
/// apertavam DEPOIS com `let _ =`. O `config.json` ainda herdava a permissao
/// do original, entao um `0644` de instalacao ficava `0644` para sempre
/// (revisao SEC de 17/09/2026, achado A4). Agora os tres passam por aqui.
///
/// O `.tmp` de uma gravacao interrompida pode ter ficado -- e ficado com a
/// permissao daquele dia. `create_new` o recusaria, e reaproveita-lo herdaria
/// a permissao velha, porque `mode` so vale na criacao. Entao ele sai antes;
/// so a ausencia dele e engolida, porque ausencia e o caso normal.
pub(crate) fn gravar_privado(caminho: &Path, corpo: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let temporario = temporario_de(caminho);
    match std::fs::remove_file(&temporario) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut arq = abrir_privado(&temporario)?;
    arq.write_all(corpo)?;
    arq.sync_all()?;
    drop(arq);
    // Troca atomica: um corte de energia no meio deixa o arquivo antigo
    // inteiro, e nao um pela metade -- que derrubaria o proximo arranque.
    std::fs::rename(&temporario, caminho)
}

/// Um servidor que a interface pode alcancar.
///
/// Nasceu como texto `"host:porta"` e CONTINUA aceitando texto solto -- essa e
/// a forma de sempre, sem cifra. O objeto `{host,porta,cifra,chave_do_fio}` e
/// o que faltava: ele carrega o pino, e sem lugar para o pino o `Remoto` nao
/// tinha como ligar o tunel com protecao contra quem esta no meio. Ligar sem
/// pino seria protecao so contra escuta passiva vendida como se fosse mais --
/// e por isso a mudanca de formato entra COM o pino junto. Ver
/// `docs/CIFRA-DO-FIO.md` §8 e §10.
#[derive(Debug, Clone)]
pub struct ServidorWeb {
    /// `"host:porta"` -- o texto EXATO que casa o destino do navegador e abre
    /// a conexao. O texto solto vira este campo direto; o objeto o remonta a
    /// partir de `host` e `porta`, para que a comparacao com o destino
    /// continue sendo textual (nada de resolver nome e comparar IP, que deixa
    /// quem controla o DNS decidir o que a lista permite).
    pub endereco: String,
    /// Falar por dentro do tunel cifrado com este destino. Ver
    /// `docs/CIFRA-DO-FIO.md`.
    ///
    /// Nasce LIGADO ([`CIFRA_DE_SAIDA_PADRAO`], 18/09/2026), **inclusive no
    /// texto solto**: um destino escrito `"host:porta"` nao tem onde escrever
    /// decisao nenhuma, e deixa-lo em claro faria a virada nao alcancar a forma
    /// mais usada das duas -- quem quer claro escreve o objeto com
    /// `"cifra": false`, que e o escape ESCRITO tambem aqui.
    ///
    /// O que ele exige continua sendo dos dois lados: o destino tem de ATENDER
    /// o aperto, e um servidor de versao anterior nao atende.
    pub cifra: bool,
    /// A chave publica que se ESPERA do destino, em hexadecimal -- o pino.
    ///
    /// Vazia com `cifra` ligada e tunel SEM pino: protege da escuta passiva e
    /// nao protege de quem esta no meio, porque o atacante apresenta a chave
    /// dele e nao ha com o que comparar. O arranque avisa exatamente isso.
    pub chave_do_fio: String,
}

impl ServidorWeb {
    fn de_texto(t: &str, saidas: &mut LeituraDasSaidas) -> ServidorWeb {
        let endereco = t.trim().to_string();
        ServidorWeb {
            // O texto solto nao tem onde escrever a decisao, entao ele e
            // sempre "de fabrica": entra no aviso do arranque ate alguem
            // trocar este item pelo objeto, com `"cifra": true` (confirmando)
            // ou `"cifra": false` (o escape).
            cifra: saidas.sem_decisao_escrita(&format!("web.servidores[{endereco:?}]")),
            endereco,
            chave_do_fio: String::new(),
        }
    }

    fn de_objeto(o: &Json, saidas: &mut LeituraDasSaidas) -> ServidorWeb {
        let host = o.texto_ou("host", "").trim().to_string();
        let porta = o.inteiro_ou("porta", PORTA_PADRAO as i64).clamp(1, 65_535) as u16;
        let endereco = format!("{host}:{porta}");
        ServidorWeb {
            cifra: saidas.cifra(o, &format!("web.servidores[{endereco:?}]")),
            endereco,
            chave_do_fio: o.texto_ou("chave_do_fio", "").trim().to_string(),
        }
    }

    /// O pino do destino ja em bytes -- ou o erro que diz o que corrigir.
    ///
    /// Mesma disciplina da [`Origem::pino_do_fio`], e pela mesma razao:
    /// `None` significa "sem pino", nunca "qualquer chave serve por engano".
    /// Hexadecimal torto vira ERRO em vez de virar `None`, senao um pino
    /// escrito errado viraria silenciosamente um tunel sem pino -- que e
    /// exatamente o estrago que o pino existe para impedir.
    pub fn pino_do_fio(&self) -> Result<Option<[u8; 32]>> {
        if self.chave_do_fio.is_empty() {
            return Ok(None);
        }
        Ok(Some(chave_de_hex(
            &self.chave_do_fio,
            &format!("web.servidores[{}].chave_do_fio", self.endereco),
        )?))
    }
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
    /// Servidores PhxSql que esta interface pode alcancar.
    ///
    /// VAZIO = so este servidor. E o padrao, e e o padrao certo: uma interface
    /// que fala com qualquer endereco e um proxy aberto de saida, e quem
    /// invadir a porta da web ganha a rede inteira junto. Cada item e um
    /// texto `"host:porta"` (em claro, como sempre foi) ou um objeto que
    /// tambem carrega o pino do tunel -- ver [`ServidorWeb`].
    pub servidores: Vec<ServidorWeb>,
    /// Ha um proxy reverso terminando TLS na frente desta porta?
    ///
    /// Nao liga nada e nao muda byte nenhum do que o servidor faz: o unico
    /// efeito e CALAR o aviso do arranque sobre porta HTTP exposta em claro
    /// (`docs/SEGURANCA.md` §7.1). Existe porque a alternativa era o aviso
    /// aparecer para sempre em toda instalacao que ja fez a coisa certa -- e
    /// aviso que sempre aparece e aviso que ninguem le, o que gasta a
    /// confianca do aviso verdadeiro.
    ///
    /// E por isso ele e uma DECLARACAO, e o texto do campo diz isso: o
    /// servidor nao tem como conferir se o proxy existe. Quem escreve `true`
    /// sem proxy nenhum na frente esta mentindo para si mesmo, e nao para o
    /// motor.
    pub atras_de_proxy: bool,
}

impl Default for Web {
    fn default() -> Self {
        Web {
            ligado: false,
            bind: format!("127.0.0.1:{PORTA_WEB_PADRAO}"),
            sessao_minutos: 60,
            servidores: Vec::new(),
            atras_de_proxy: false,
        }
    }
}

impl Web {
    fn de_json(j: &Json, saidas: &mut LeituraDasSaidas) -> Web {
        let padrao = Web::default();
        match j.campo("web") {
            None => padrao,
            Some(w) => Web {
                ligado: w.booleano_ou("ligado", false),
                bind: w.texto_ou("bind", &padrao.bind).to_string(),
                sessao_minutos: w
                    .inteiro_ou("sessao_minutos", padrao.sessao_minutos as i64)
                    .max(1) as u64,
                servidores: Web::servidores_de(w, saidas),
                atras_de_proxy: w.booleano_ou("atras_de_proxy", padrao.atras_de_proxy),
            },
        }
    }

    /// Le a lista `web.servidores`, que aceita as DUAS formas ao mesmo tempo.
    ///
    /// Texto solto = como sempre foi, sem cifra. Objeto = a forma que carrega o
    /// pino. Aceitar as duas na mesma lista e o que faz a mudanca de formato
    /// ser retrocompativel: um `config.json` de antes desta rodada continua
    /// valendo byte a byte, e so quem precisa do tunel troca aquele item por
    /// objeto. O `textos` de antes descartava calado tudo o que nao fosse
    /// texto -- por isso ele nao servia mais.
    fn servidores_de(w: &Json, saidas: &mut LeituraDasSaidas) -> Vec<ServidorWeb> {
        w.campo("servidores")
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .map(|e| match e {
                        Json::Texto(t) => ServidorWeb::de_texto(t, saidas),
                        _ => ServidorWeb::de_objeto(e, saidas),
                    })
                    .collect()
            })
            .unwrap_or_default()
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
        self.servidor(alvo).is_some()
    }

    /// O servidor da lista que casa este destino, ou `None`.
    ///
    /// A conferencia e a mesma do `servidor_permitido` -- endereco vazio nunca
    /// casa --, e e por aqui que o `Remoto` descobre se o destino pede tunel e
    /// com qual pino. Um so ponto de casamento: se `servidor_permitido` deixou
    /// passar, este devolve o item; se recusou, este devolve `None`, e as duas
    /// respostas nunca divergem porque saem da mesma comparacao.
    pub fn servidor(&self, alvo: &str) -> Option<&ServidorWeb> {
        let d = alvo.trim();
        if d.is_empty() {
            return None;
        }
        self.servidores.iter().find(|s| s.endereco.trim() == d)
    }
}

/// O webservice REST, e o explorador da especificacao ao lado dele.
///
/// # Duas portas, e por que
///
/// O REST escuta na 6000 e o explorador na 7000, cada um com o proprio
/// `ligado`. Quem sobe numa placa quer o webservice e nao quer o visualizador:
/// com uma porta so, desligar o segundo exigiria desligar o primeiro.
///
/// # As duas nascem DESLIGADAS
///
/// Regra petrea da casa: guarda nova -- e porta nova -- entra **pedida**. Um
/// servidor que ja roda hoje nao pode passar a expor porta nenhuma so porque
/// alguem atualizou o binario: isso e abrir superficie de ataque sem ninguem
/// pedir. O teste que trava isso e `config_sem_a_secao_rest_nao_escuta`, e ele
/// e do comportamento VELHO, que e o que mais importa.
///
/// # O filtro de tabelas SO ESTREITA
///
/// [`Self::tabelas`] **nao e um sistema de permissao**. Ela e um filtro
/// aplicado ANTES do portao, nunca no lugar dele: tabela fora da lista nao
/// existe para o REST, e tabela dentro da lista continua passando pelo
/// `despachar` e pelo direito do usuario exatamente como sempre. Duas verdades
/// sobre direito de acesso e onde nasce o furo -- a casa ja pagou quatro deles
/// quando o portao passou a olhar um campo que algumas operacoes nao tem.
#[derive(Clone)]
pub struct Rest {
    pub ligado: bool,
    /// Endereco de escuta do REST. Padrao: so o proprio computador.
    pub bind: String,
    /// O nome deste webservice. Vai para o `info.title` da especificacao e
    /// para o cabecalho do explorador -- e so para isso: nome nao e portao.
    pub nome: String,
    /// O banco que este webservice atende. Vazio = nao estreita nada.
    ///
    /// Preenchido, ele faz duas coisas e nenhuma delas alarga: preenche o
    /// `database` do pedido que veio sem ele (conveniencia de quem publica UM
    /// banco) e recusa, como inexistente, o pedido que nomeia outro.
    pub database: String,
    /// As tabelas que este webservice expoe. Vazia = nao estreita nada.
    pub tabelas: Vec<String>,
    /// O segredo da porta REST. Vazio = vale o `token` do protocolo.
    ///
    /// Preenchido, ele SUBSTITUI o token do protocolo **nesta porta**: o
    /// `Bearer` tem de ser este, e o token do protocolo nao abre o REST. Nao e
    /// uma identidade nem um poder -- e a chave da porta da rede, igual ao
    /// outro, e quem entra continua sendo quem faz `login`.
    pub token: String,
    /// O explorador da especificacao escuta?
    pub swagger_ligado: bool,
    /// Endereco de escuta do explorador.
    pub swagger_bind: String,
    /// Ha um proxy reverso terminando TLS na frente das DUAS portas do REST?
    ///
    /// Mesmo campo, mesmo efeito e mesma ressalva do [`Web::atras_de_proxy`]:
    /// so cala o aviso do arranque. Vale para `bind` e para `swagger_bind`
    /// juntos porque as duas sobem do mesmo bloco e do mesmo operador -- um
    /// terceiro interruptor so para o explorador da especificacao seria
    /// configuracao que ninguem ajusta separada.
    pub atras_de_proxy: bool,
}

impl Rest {
    /// O `Bearer` da porta REST confere com o segredo dela? Em tempo
    /// constante, como o token do protocolo -- o REST comparava com `==`
    /// enquanto o portao 1 ja usava `token_confere` (achado lateral do papel
    /// J, 17/09/2026: o conserto que nao voltou ao irmao).
    pub fn token_confere(&self, oferecido: &str) -> bool {
        phxsql_core::hash::iguais_em_tempo_constante(self.token.as_bytes(), oferecido.as_bytes())
    }
}

/// `Debug` a mao: o token do REST e a chave da porta da rede, e substitui o do
/// protocolo nesta porta.
impl std::fmt::Debug for Rest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Rest {
            ligado,
            bind,
            nome,
            database,
            tabelas,
            token: _,
            swagger_ligado,
            swagger_bind,
            atras_de_proxy,
        } = self;
        f.debug_struct("Rest")
            .field("ligado", ligado)
            .field("bind", bind)
            .field("nome", nome)
            .field("database", database)
            .field("tabelas", tabelas)
            .field("token", &"(oculto)")
            .field("swagger_ligado", swagger_ligado)
            .field("swagger_bind", swagger_bind)
            .field("atras_de_proxy", atras_de_proxy)
            .finish()
    }
}

impl Default for Rest {
    fn default() -> Self {
        Rest {
            ligado: false,
            bind: format!("127.0.0.1:{PORTA_REST_PADRAO}"),
            nome: String::new(),
            database: String::new(),
            tabelas: Vec::new(),
            token: String::new(),
            swagger_ligado: false,
            swagger_bind: format!("127.0.0.1:{PORTA_SWAGGER_PADRAO}"),
            atras_de_proxy: false,
        }
    }
}

impl Rest {
    fn de_json(j: &Json) -> Rest {
        let padrao = Rest::default();
        match j.campo("rest") {
            None => padrao,
            Some(r) => Rest {
                ligado: r.booleano_ou("ligado", false),
                bind: r.texto_ou("bind", &padrao.bind).trim().to_string(),
                nome: r.texto_ou("nome", "").trim().to_string(),
                database: r.texto_ou("database", "").trim().to_string(),
                tabelas: r
                    .textos("tabelas")
                    .into_iter()
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect(),
                token: r.texto_ou("token", "").trim().to_string(),
                swagger_ligado: r.booleano_ou("swagger_ligado", false),
                swagger_bind: r
                    .texto_ou("swagger_bind", &padrao.swagger_bind)
                    .trim()
                    .to_string(),
                atras_de_proxy: r.booleano_ou("atras_de_proxy", padrao.atras_de_proxy),
            },
        }
    }

    /// O que a tela ve. O TOKEN NAO SAI DAQUI -- so o fato de existir um.
    ///
    /// Senha nunca em texto puro, e um segredo de porta e senha: mandar o
    /// token na resposta o poria no `localStorage` de quem abriu a tela, no
    /// log do proxy e na captura de tela do suporte.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ligado", Json::Bool(self.ligado)),
            ("bind", Json::texto_de(&self.bind)),
            ("nome", Json::texto_de(&self.nome)),
            ("database", Json::texto_de(&self.database)),
            (
                "tabelas",
                Json::Lista(self.tabelas.iter().map(Json::texto_de).collect()),
            ),
            ("token_proprio", Json::Bool(!self.token.is_empty())),
            ("swagger_ligado", Json::Bool(self.swagger_ligado)),
            ("swagger_bind", Json::texto_de(&self.swagger_bind)),
            ("atras_de_proxy", Json::Bool(self.atras_de_proxy)),
        ])
    }

    pub fn endereco(&self) -> Result<SocketAddr> {
        endereco_de(&self.bind).map_err(|e| PhxError::Esquema(format!("rest.bind: {e}")))
    }

    pub fn endereco_do_swagger(&self) -> Result<SocketAddr> {
        endereco_de(&self.swagger_bind)
            .map_err(|e| PhxError::Esquema(format!("rest.swagger_bind: {e}")))
    }

    /// O nome que a especificacao e o explorador mostram.
    pub fn titulo(&self) -> String {
        if self.nome.is_empty() {
            "PhxSql".to_string()
        } else {
            self.nome.clone()
        }
    }

    /// Esta tabela existe para o REST?
    ///
    /// Lista vazia libera tudo -- e e o padrao, byte a byte o comportamento de
    /// quem nao preencheu nada. Compara o nome como veio E sem o schema, para
    /// `vendas.clientes` casar com `clientes` na lista: quem escreve a lista
    /// esta pensando em tabela, nao em caminho.
    pub fn tabela_exposta(&self, nome: &str) -> bool {
        if self.tabelas.is_empty() {
            return true;
        }
        let alvo = nome.trim();
        let curto = alvo.rsplit('.').next().unwrap_or(alvo);
        self.tabelas
            .iter()
            .any(|t| t.eq_ignore_ascii_case(alvo) || t.eq_ignore_ascii_case(curto))
    }

    /// Este banco existe para o REST? Vazio no arquivo libera todos.
    pub fn database_exposto(&self, nome: &str) -> bool {
        self.database.is_empty()
            || nome.trim().is_empty()
            || self.database.eq_ignore_ascii_case(nome.trim())
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
    /// Threads HTTP ao mesmo tempo, somando as tres portas (interface web,
    /// REST e explorador da API). Zero = sem teto, que era o comportamento
    /// ate a 0.18: uma thread por pedido, sem limite.
    ///
    /// E um teto DIFERENTE do `conexoes_max`, e de proposito: a porta de
    /// dados recusa na hora (consenso PostgreSQL/MySQL/MariaDB para
    /// `max_connections`), e a web ESPERA um pouco antes de recusar -- o molde
    /// e o de um servidor HTTP (workers + backlog), porque do outro lado ha
    /// um navegador que refaz o pedido sozinho e uma pessoa que so ve o
    /// atraso. Ver `fila_web_ms`.
    pub conexoes_web_max: usize,
    /// Quanto um pedido HTTP espera por uma thread quando todas estao
    /// ocupadas, em ms. Estourou, o pedido recebe 503 com `Retry-After`.
    ///
    /// Zero quer dizer **nao espere**: recusa na hora, como a porta de dados.
    pub fila_web_ms: u64,
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
            // O mesmo numero da porta de dados, e nao um numero medido para a
            // web: a bancada `enxurrada-web.py` mede o que ele segura, e quem
            // tiver um numero melhor muda aqui com a medicao ao lado.
            conexoes_web_max: 64,
            // Dois segundos e o que um navegador ainda sente como «demorou»
            // e nao como «caiu»; acima disso a pessoa ja clicou de novo.
            fila_web_ms: 2_000,
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
        PerfilEmDisco::de_secao(j, "profiler", PerfilEmDisco::default())
    }

    /// A mesma leitura (`arquivo_mib`/`arquivos`), generalizada por SECAO e
    /// por PADRAO -- pedido 228: `acessos.log` e `diretivas.log` reaproveitam
    /// este tipo para o rodizio deles, e nao inventam outro. O padrao muda
    /// por chamador porque a regra da casa exige: o Profiler NASCE ligado
    /// (decisao propria, documentada acima), mas `acessos`/`diretivas` tem
    /// de nascer com `arquivo_mib: 0` -- guarda nova entra pedida, e um
    /// arquivo que ja existe em producao nao pode passar a girar sozinho.
    fn de_secao(j: &Json, secao: &str, padrao: PerfilEmDisco) -> PerfilEmDisco {
        let Some(c) = j.campo(secao) else {
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

    /// O padrao de `acessos` e `diretivas`: sem rodizio, o comportamento de
    /// sempre -- ver a nota de `de_secao` sobre por que ele NAO e o mesmo
    /// `Default` do Profiler.
    fn sem_rodizio() -> PerfilEmDisco {
        PerfilEmDisco {
            arquivo_mib: 0,
            arquivos: 0,
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
        // Compoe duas metades testadas em separado (pedido 234): o calculo
        // de `self.nucleos()` prova sozinho em `testes_recursos` (sem tocar
        // `paralelo::TETO`), e o global `definir_teto`/`nucleos()` prova
        // sozinho em `phxsql_core::paralelo::tests` (sem nada mais mexer nele
        // naquele binario). Nenhum teste liga as duas pontas AQUI de proposito:
        // `aplicar()` roda dentro de `Config::ler`, chamado por uma dezena de
        // testes deste arquivo em paralelo, e um teste que lesse `paralelo::
        // nucleos()` logo depois desta linha estaria apostando contra os
        // vizinhos -- foi exatamente essa aposta que caiu 4 vezes em 200
        // corridas antes do conserto. A linha revisada: `self.nucleos()` ja
        // aplica o `cpu_percentual` e nunca devolve zero, e `definir_teto`
        // so guarda o numero -- nao ha conversao nem faixa para errar aqui.
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
            conexoes_web_max: r
                .inteiro_ou("conexoes_web_max", padrao.conexoes_web_max as i64)
                .max(0) as usize,
            fila_web_ms: r
                .inteiro_ou("fila_web_ms", padrao.fila_web_ms as i64)
                .max(0) as u64,
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
            (
                "conexoes_web_max",
                Json::de_u64(self.conexoes_web_max as u64),
            ),
            ("fila_web_ms", Json::de_u64(self.fila_web_ms)),
            ("usuarios_max", Json::de_u64(self.usuarios_max as u64)),
        ])
    }
}

#[derive(Clone)]
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
    /// Webservice REST com OpenAPI. Ver [`Rest`].
    pub rest: Rest,
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
    /// A chave mestra que cifra as credenciais do `dblink.json` (pedido 372).
    /// Ver [`CifraDoDblink`].
    pub cifra_do_dblink: CifraDoDblink,
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
    /// O rodizio do `acessos.log`. Pedido 228 -- mesmo tipo do Profiler,
    /// padrao DESLIGADO (ver [`PerfilEmDisco::sem_rodizio`]).
    pub acessos: PerfilEmDisco,
    /// O rodizio do `diretivas.log`. Pedido 228, mesma nota de `acessos`.
    pub diretivas: PerfilEmDisco,
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

/// `Debug` a mao: o `token` e o segredo exigido em TODO pedido -- o portao 1
/// deste servidor. Um `dbg!(&config)` num diagnostico apressado o jogaria no
/// log, e com ele quem le o log fala com o banco.
///
/// As pecas de dentro que carregam segredo escrevem o proprio `Debug` (a
/// [`Cifra`], a [`CifraFio`], o [`Email`], a [`Origem`], o [`Cluster`], o
/// [`Rest`]), entao aqui basta o campo proprio.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Config {
            bind,
            base,
            token: _,
            max_linhas,
            log_acessos,
            ips_permitidos,
            conexoes_max,
            recursos,
            timeout_s,
            somente_leitura,
            espelho,
            replicacao,
            cluster,
            cadastro,
            politica,
            blacklist,
            web,
            rest,
            backup,
            alertas,
            dblink,
            cifra_do_dblink,
            jobs,
            cifra,
            cifra_fio,
            lgpd,
            telemetria,
            profiler,
            acessos,
            diretivas,
            idioma,
            estranhas,
            avisos,
            caminho,
        } = self;
        f.debug_struct("Config")
            .field("bind", bind)
            .field("base", base)
            .field("token", &"(oculto)")
            .field("max_linhas", max_linhas)
            .field("log_acessos", log_acessos)
            .field("ips_permitidos", ips_permitidos)
            .field("conexoes_max", conexoes_max)
            .field("recursos", recursos)
            .field("timeout_s", timeout_s)
            .field("somente_leitura", somente_leitura)
            .field("espelho", espelho)
            .field("replicacao", replicacao)
            .field("cluster", cluster)
            .field("cadastro", cadastro)
            .field("politica", politica)
            .field("blacklist", blacklist)
            .field("web", web)
            .field("rest", rest)
            .field("backup", backup)
            .field("alertas", alertas)
            .field("dblink", dblink)
            .field("cifra_do_dblink", cifra_do_dblink)
            .field("jobs", jobs)
            .field("cifra", cifra)
            .field("cifra_fio", cifra_fio)
            .field("lgpd", lgpd)
            .field("telemetria", telemetria)
            .field("profiler", profiler)
            .field("acessos", acessos)
            .field("diretivas", diretivas)
            .field("idioma", idioma)
            .field("estranhas", estranhas)
            .field("avisos", avisos)
            .field("caminho", caminho)
            .finish()
    }
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
const CAMPOS_CONHECIDOS: [&str; 31] = [
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
    "rest",
    "backup",
    "alertas",
    "dblink",
    "cifra_do_dblink",
    "jobs",
    "cifra",
    "cifra_fio",
    "idioma",
    "lgpd",
    "telemetria",
    "profiler",
    "acessos",
    "diretivas",
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
const SECOES_CONHECIDAS: [(&str, &[&str]); 16] = [
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
            "conexoes_web_max",
            "fila_web_ms",
            "carga_prazo_min",
            "transacao_prazo_min",
            "transacao_max_linhas",
            "transacao_lock_timeout_ms",
            "transacao_statement_ms",
            "usuarios_max",
            "diario_volume_mib",
        ],
    ),
    (
        "web",
        &[
            "ligado",
            "bind",
            "sessao_minutos",
            "servidores",
            "atras_de_proxy",
        ],
    ),
    (
        "rest",
        &[
            "ligado",
            "bind",
            "nome",
            "database",
            "tabelas",
            "token",
            "swagger_ligado",
            "swagger_bind",
            "atras_de_proxy",
        ],
    ),
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
            // Pedido 249: a sonda de saude do disco e o canal de SMS.
            "disco",
            "sms",
        ],
    ),
    (
        "alertas.disco",
        &["ligado", "checar_segundos", "repetir_minutos", "lento_ms"],
    ),
    ("alertas.sms", &["ligado", "numeros", "gateway_email"]),
    (
        "alertas.email",
        &[
            "ligado",
            // Da frente dos jobs: liga o aviso por e-mail de job que falhou ou
            // parou. Sem ele na lista, o verificador novo -- que passou a olhar
            // o INTERIOR das secoes -- acusava campo estranho num exemplo que
            // esta certo.
            "avisar_jobs",
            // Da frente da seguranca: liga o aviso por e-mail da violacao
            // grave que bloqueia um IP. Mesmo motivo do de cima.
            "avisar_seguranca",
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
            "tabelas",
        ],
    ),
    (
        "cifra_fio",
        &[
            "ligada",
            "exigir",
            "exigir_amarra",
            "chave_privada",
            "chave_privada_env",
            "arquivo",
        ],
    ),
    // `senha_mestra` e `chave_mestra` ESTAO na lista, e nao por serem
    // aceitos: sao RECUSADOS pelo `validar`, com o motivo. Deixa-los de fora
    // somaria um «campo desconhecido» a recusa, e o aviso errado mandaria
    // procurar erro de digitacao onde ha uma decisao recusada.
    (
        "cifra_do_dblink",
        &[
            "senha_mestra_env",
            "senha_mestra_arquivo",
            "chave_mestra_env",
            "chave_mestra_arquivo",
            "iteracoes",
            "senha_mestra",
            "chave_mestra",
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
    // Pedido 228: o rodizio de `acessos.log` e `diretivas.log`, mesmo par de
    // campos do `profiler` acima.
    ("acessos", &["arquivo_mib", "arquivos"]),
    ("diretivas", &["arquivo_mib", "arquivos"]),
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
            rest: Rest::default(),
            backup: Backup::default(),
            alertas: Alertas::default(),
            dblink: PathBuf::from("dblink.json"),
            cifra_do_dblink: CifraDoDblink {
                iteracoes: phxsql_store::cofre::ITERACOES_PADRAO,
                ..CifraDoDblink::default()
            },
            jobs: PathBuf::from("jobs.json"),
            cifra: Cifra::default(),
            cifra_fio: CifraFio::default(),
            lgpd: Lgpd::default(),
            telemetria: Painel::default(),
            profiler: PerfilEmDisco::default(),
            acessos: PerfilEmDisco::sem_rodizio(),
            diretivas: PerfilEmDisco::sem_rodizio(),
            idioma: String::new(),
            estranhas: Vec::new(),
            avisos: Vec::new(),
            caminho: None,
        }
    }
}

/// Duas portas colidem de verdade, ou uma delas e' "0" (o sistema escolhe)?
///
/// Pedido 401: quando o `bind` pede porta 0, o texto de DOIS campos pode ser
/// identico (`"127.0.0.1:0"` em `bind` e em `web.bind`, por exemplo) sem que
/// haja colisao nenhuma -- cada `TcpListener::bind` com porta 0 recebe um
/// numero DIFERENTE do sistema operacional, sempre; nunca o mesmo dois vezes
/// na mesma maquina. A guarda de endereco repetido existe para pegar o erro
/// de digitacao (duas portas fixas iguais), e comparar dois "0" por igualdade
/// literal recusaria exatamente o padrao que fecha a corrida do pedido 401
/// (pedir porta 0 em todo campo e ler a REAL de volta depois do `bind`).
fn enderecos_colidem(a: SocketAddr, b: SocketAddr) -> bool {
    a.port() != 0 && b.port() != 0 && a == b
}

impl Config {
    /// Le o `config.json` do caminho informado -- ou o `config.phz` ao lado
    /// dele (pedido 450).
    ///
    /// O caminho PEDIDO nao e necessariamente o lido: do par claro/`.phz`,
    /// vale o que existe, e os dois presentes recusam o arranque (a regra em
    /// [`crate::config_phz`]). O `caminho` guardado no `Config` e o do arquivo
    /// LIDO, e e por ele que as gravacoes decidem a forma: quem subiu de `.phz`
    /// grava `.phz`, quem subiu em claro grava em claro.
    pub fn ler(caminho: impl AsRef<Path>) -> Result<Config> {
        let lido = crate::config_phz::resolver(caminho.as_ref())?;
        let caminho = lido.as_path();
        let texto = crate::config_phz::ler_texto(caminho)?;
        let json = Json::analisar(&texto)?;
        let mut c = Config::de_json(&json)?;
        c.caminho = Some(caminho.to_path_buf());
        // Caminhos relativos valem a partir do diretorio do config.json, NUNCA
        // do diretorio de trabalho de quem subiu o processo -- UMA funcao so
        // para os seis campos (pedido 225). Antes deste conserto, `dblink` e
        // `jobs` nao estavam nesta lista: entraram no `Config` DEPOIS deste
        // bloco ja existir (27/08/2026, o servidor original) e ninguem voltou
        // aqui -- a mesma armadilha da peca nova no fim de uma lista que ja
        // pegou o `EXTENSOES_TODAS` do `phxsql-store` (pedido 213) e a lista
        // de KiB do rodape do dossie. Um `phxsqld` iniciado de fora do
        // diretorio do config espalhava `jobs.json` (e `jobs.log`, ao lado) no
        // cwd do processo -- foi assim que a bancada `bancada/proibidos/
        // provar.py` deixou os dois na raiz do repositorio.
        c.base = resolver_caminho_do_config(&c.base, c.caminho.as_deref());
        c.log_acessos = resolver_caminho_do_config(&c.log_acessos, c.caminho.as_deref());
        c.blacklist = resolver_caminho_do_config(&c.blacklist, c.caminho.as_deref());
        c.dblink = resolver_caminho_do_config(&c.dblink, c.caminho.as_deref());
        c.jobs = resolver_caminho_do_config(&c.jobs, c.caminho.as_deref());
        c.backup.destino = resolver_caminho_do_config(&c.backup.destino, c.caminho.as_deref());
        // DEPOIS dos caminhos, porque a recusa da chave mestra compara com
        // eles: a pasta dos dados e a do `dblink.json` so estao certas daqui
        // para baixo.
        c.cifra_do_dblink
            .resolver(c.caminho.as_deref(), &c.base, &c.dblink);
        c.avisar_o_cadastro_do_dblink();
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
        // A leitura dos interruptores de SAIDA atravessa tres secoes do
        // arquivo (`replicacao`, `cluster` e `web`), e o que ela junta e o que
        // o `Config` pronto nao teria mais como saber: quem escreveu a decisao
        // e quem herdou o padrao. Ver [`LeituraDasSaidas`].
        let mut saidas = LeituraDasSaidas::default();
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
                            .map(|o| {
                                let nome = o.texto_ou("nome", "origem").to_string();
                                let host = o.texto_ou("host", "127.0.0.1").to_string();
                                let porta = o.inteiro_ou("porta", PORTA_PADRAO as i64) as u16;
                                // O rotulo nomeia a CONEXAO, e nao so o campo:
                                // uma lista de interruptores nao diz qual
                                // replicacao vai parar.
                                let cifra = saidas.cifra(
                                    o,
                                    &format!(
                                        "replicacao.origens[{nome:?}] (source {host}:{porta})"
                                    ),
                                );
                                Origem {
                                    nome,
                                    host,
                                    porta,
                                    token: o.texto_ou("token", "").to_string(),
                                    databases: o.textos("databases"),
                                    reconectar_em: o.inteiro_ou("reconectar_em", 10).max(1) as u64,
                                    usuario: o.texto_ou("usuario", "").trim().to_string(),
                                    senha_hash: o.texto_ou("senha_hash", "").trim().to_string(),
                                    senha: o.texto_ou("senha", "").to_string(),
                                    cada_minutos: o.inteiro_ou("cada_minutos", 0).max(0) as u64,
                                    hora: o.texto_ou("hora", "").trim().to_string(),
                                    cifra,
                                    chave_do_fio: o.texto_ou("chave_do_fio", "").trim().to_string(),
                                }
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
        let cluster = Cluster::de_json(j, &rep.id_servidor, &mut saidas)?;
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
        let mut c = Config {
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
            web: Web::de_json(j, &mut saidas),
            rest: Rest::de_json(j),
            backup: Backup::de_json(j)?,
            alertas: Alertas::de_json(j)?,
            dblink: PathBuf::from(j.texto_ou("dblink", "dblink.json")),
            cifra_do_dblink: CifraDoDblink::de_json(j),
            jobs: PathBuf::from(j.texto_ou("jobs", "jobs.json")),
            cifra: Cifra::de_json(j),
            cifra_fio: CifraFio::de_json(j),
            lgpd: Lgpd::de_json(j),
            telemetria: Painel::de_json(j, &mut avisos),
            profiler: PerfilEmDisco::de_json(j),
            acessos: PerfilEmDisco::de_secao(j, "acessos", PerfilEmDisco::sem_rodizio()),
            diretivas: PerfilEmDisco::de_secao(j, "diretivas", PerfilEmDisco::sem_rodizio()),
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
        };
        // A senha da cifra e a senha de um administrador: AVISA, nao recusa.
        //
        // Guarda nova entra PEDIDA, nao imposta. Um servidor que hoje tem as
        // duas iguais nao pode parar de subir por causa de uma regra escrita
        // depois -- o estrago seria maior que o furo, e o dono ficaria com um
        // banco no chao para consertar uma senha. A recusa vale para senha de
        // banco DEFINIDA daqui em diante, pela operacao que ainda nao existe;
        // o que ja esta gravado continua abrindo, e apenas aparece.
        //
        // A mensagem nao nomeia o administrador -- ver
        // `Cifra::senha_e_de_algum_administrador`.
        // Sem a variavel nao ha senha a comparar: a falta tem aviso proprio,
        // logo abaixo, e o `aplicar` recusa o arranque com o nome dela.
        if c.cifra.ligada
            && c.cifra
                .senha()
                .is_ok_and(|s| Cifra::senha_e_de_algum_administrador(s, &c.cadastro))
        {
            c.avisos.push(
                "cifra.senha e igual a senha de um administrador deste servidor. \
                 A senha do banco VIAJA (o servidor precisa dela em claro para \
                 derivar a chave); a de administracao nao viaja, porque o \
                 desafio-resposta manda uma prova. Iguais, quem escutar o fio \
                 vira administrador -- separadas, no pior caso le um banco."
                    .to_string(),
            );
        }
        c.avisar_os_segredos_que_faltam();
        c.avisar_o_que_viaja_em_claro();
        c.avisar_a_cifra_de_fabrica_das_saidas(saidas);
        Ok(c)
    }

    /// Os avisos do arranque sobre o `dblink.json` -- credencial em texto puro
    /// e variavel declarada que falta (pedido 372) --, pela MESMA lista dos
    /// outros avisos, que o `main` imprime.
    ///
    /// # Por que aqui, e nao onde o cadastro abre de verdade
    ///
    /// O cadastro abre no `Servidor::novo`, e ali nao ha lista de avisos: um
    /// segundo jeito de avisar, ao lado deste, seria o mecanismo repetido que
    /// a regra «funcao e comando vem do mesmo motor» proibe. Entao o `ler` so
    /// PERGUNTA ao cadastro ([`crate::dblink::Registro::avisos`], onde a regra
    /// mora) e poe a resposta na lista de sempre.
    ///
    /// Arquivo torto aqui e silencio, e nao erro: quem recusa o arranque por
    /// ele continua sendo o `Servidor::novo`, com a mensagem dele. Duas
    /// recusas do mesmo arquivo, com textos diferentes, mandariam procurar em
    /// dois lugares.
    fn avisar_o_cadastro_do_dblink(&mut self) {
        if let Ok(cadastro) =
            crate::dblink::Registro::abrir_com(&self.dblink, &self.cifra_do_dblink)
        {
            self.avisos.extend(cadastro.avisos());
        }
    }

    /// O aviso do arranque para cada `*_env` do `config.json` que nomeia
    /// variavel que este processo nao tem (pedido 372).
    ///
    /// # Aviso, e nao recusa -- com uma excecao que ja recusava
    ///
    /// A falta vira ERRO em quem precisa do valor ([`Segredo::valor`]), e so
    /// ali: o rele recusa o envio, o aperto de mao recusa o tunel. Recusar
    /// aqui derrubaria o `--usuarios` de um administrador que roda a CLI sem o
    /// ambiente do servico. A excecao e o cofre ligado: o `Cifra::aplicar`
    /// continua recusando o arranque, como ja recusava a senha vazia -- agora
    /// dizendo qual variavel faltou.
    ///
    /// Fala so com quem declarou e nao tem. Quem escreveu o segredo no arquivo,
    /// ou exportou a variavel, nao ouve nada daqui.
    fn avisar_os_segredos_que_faltam(&mut self) {
        let donos = [
            (
                self.alertas.email.senha.falta(),
                "o aviso por e-mail vai falhar ao se autenticar no rele",
            ),
            (
                self.cifra.senha.falta(),
                "com cifra.ligada, o arranque recusa -- o cofre nao abre sem a senha",
            ),
            (
                self.cifra_fio.chave_privada.falta(),
                "o aperto de mao da cifra do fio vai RECUSAR -- e a chave NAO foi \
                 trocada pela do arquivo, que e o que acontecia antes",
            ),
        ];
        let novos: Vec<String> = donos
            .into_iter()
            .filter_map(|(falta, consequencia)| falta.map(|f| format!("{f}. {consequencia}.")))
            .collect();
        self.avisos.extend(novos);
    }

    /// Os avisos do arranque sobre o que sai desta maquina em texto puro.
    ///
    /// # Por que AVISO, e nunca recusa
    ///
    /// Porta HTTP aberta para a rede e configuracao legitima -- e o desenho
    /// normal de quem termina TLS num proxy reverso (`docs/SEGURANCA.md`
    /// §7.1). Recusar derrubaria toda instalacao que ja faz a coisa certa, e
    /// guarda nova entra PEDIDA: o que muda de padrao e o endereco de fabrica
    /// (`127.0.0.1`), nao o direito de escrever outro.
    ///
    /// # Por que as TRES portas HTTP, e nao so a web
    ///
    /// `web.bind`, `rest.bind` e `rest.swagger_bind` chamam as mesmas funcoes
    /// na mesma ordem -- `endereco()` e `TcpListener::bind` no arranque --, e
    /// carregam o mesmo dado em claro. Avisar so a primeira deixaria o irmao
    /// para tras, que e o defeito que esta casa ja pagou tres vezes num dia.
    ///
    /// # E o segundo aviso, que mudou de assunto no pedido 370
    ///
    /// Ate 18/09/2026 ele dizia o ALCANCE: `cifra_fio.exigir` recusava texto
    /// claro num lugar so -- o laco da porta de DADOS --, e quem o ligava com
    /// uma porta HTTP no ar fechava um fio e deixava o outro aberto, com o
    /// mesmo token e o mesmo login. Isso acabou: as portas HTTP recusam
    /// (`Servidor::portao_de_rede_http`).
    ///
    /// O terceiro aviso -- o das SAIDAS -- morava aqui e mudou de casa junto
    /// com o assunto dele: ver [`Config::avisar_a_cifra_de_fabrica_das_saidas`].
    /// Este aqui fala do que ENTRA; aquele, do que SAI.
    ///
    /// O aviso continua existindo porque a CONSEQUENCIA e dura e silenciosa:
    /// com a exigencia ligada -- que agora e o padrao -- uma porta HTTP sem
    /// proxy declarado nao atende mais nada, e quem so trocou o binario veria
    /// a tela morrer sem saber por que. Ele nomeia as portas e as duas saidas
    /// escritas.
    ///
    /// E ele NAO sai para quem declarou o proxy: aviso que aparece para sempre
    /// numa instalacao correta e aviso que ninguem le, e isso gasta a
    /// confianca do aviso verdadeiro.
    fn avisar_o_que_viaja_em_claro(&mut self) {
        let mut novos: Vec<String> = Vec::new();
        let portas: [(&str, bool, &str, bool, &str); 3] = [
            (
                "web.bind",
                self.web.ligado,
                self.web.bind.as_str(),
                self.web.atras_de_proxy,
                "web",
            ),
            (
                "rest.bind",
                self.rest.ligado,
                self.rest.bind.as_str(),
                self.rest.atras_de_proxy,
                "rest",
            ),
            (
                "rest.swagger_bind",
                self.rest.swagger_ligado,
                self.rest.swagger_bind.as_str(),
                self.rest.atras_de_proxy,
                "rest",
            ),
        ];
        let mut sem_proxy: Vec<&str> = Vec::new();
        for (rotulo, ligada, bind, atras_de_proxy, secao) in portas {
            if !ligada {
                continue;
            }
            if !atras_de_proxy {
                sem_proxy.push(rotulo);
            }
            if atras_de_proxy || !escuta_fora_da_maquina(bind) {
                continue;
            }
            novos.push(format!(
                "{rotulo} esta em {bind}, que atende de FORA desta \
                 maquina, e HTTP e texto puro: senha, token e dado viajam \
                 legiveis para quem estiver no caminho. O PhxSql nao termina \
                 TLS (petrea das zero dependencias) -- ponha um proxy reverso \
                 terminando TLS na frente e devolva esta porta para \
                 127.0.0.1. Se o proxy ja esta la, escreva \"atras_de_proxy\": \
                 true na secao {secao} para calar este aviso. Receita em \
                 docs/SEGURANCA.md 7.1."
            ));
        }
        if self.cifra_fio.exigir && !sem_proxy.is_empty() {
            novos.push(format!(
                "cifra_fio.exigir esta ligado, e por isso estas portas HTTP \
                 RECUSAM todo pedido enquanto ninguem declarar o proxy: {}. \
                 HTTP e texto puro e este servidor nao termina TLS (petrea das \
                 zero dependencias) -- ponha um proxy reverso terminando TLS na \
                 frente e escreva \"atras_de_proxy\": true na secao web ou rest, \
                 ou escreva \"exigir\": false em cifra_fio para voltar ao claro. \
                 Receita em docs/SEGURANCA.md 7.1.",
                sem_proxy.join(", ")
            ));
        }
        self.avisos.extend(novos);
    }

    /// O aviso das SAIDAS -- e ele mudou de assunto com a virada do padrao.
    ///
    /// # O que ele dizia, e por que virou mentira pela metade
    ///
    /// Ate 18/09/2026 ele dizia: *"estas saidas estao em claro, e um PhxSql
    /// desta versao do outro lado vai RECUSAR"*. Fazia sentido enquanto as
    /// saidas nasciam desligadas -- o claro era o esquecimento, e o aviso
    /// nomeava o esquecimento. Com [`CIFRA_DE_SAIDA_PADRAO`] ligado, saida em
    /// claro so existe quando alguem ESCREVEU `"cifra": false`, e avisar quem
    /// escreveu o escape e avisar contra a propria decisao dele, todo arranque.
    ///
    /// # O que ele diz agora, e para quem
    ///
    /// A consequencia que sobrou: a saida vai PEDIR o aperto, e um PhxSql
    /// anterior a esta data nao o atende -- a conexao para. O servidor nao tem
    /// como saber a versao do outro lado, entao ele fala com quem nao escreveu
    /// decisao nenhuma, que e exatamente a populacao que a virada pegou de
    /// surpresa.
    ///
    /// E cala para as DUAS decisoes escritas -- `true` (eu sei, o outro lado
    /// fala) e `false` (o escape) --, porque o que se cobra aqui e a decisao
    /// registrada, e nao um dos valores. Aviso que aparece para sempre numa
    /// instalacao decidida e aviso que ninguem le, e isso gasta a confianca do
    /// aviso verdadeiro.
    ///
    /// Servidor isolado nao ganha nada: sem saida configurada nao ha conexao
    /// que possa parar.
    fn avisar_a_cifra_de_fabrica_das_saidas(&mut self, saidas: LeituraDasSaidas) {
        // O torto primeiro: ele e sobre UM campo que o servidor nao entendeu,
        // e some assim que alguem o corrigir.
        self.avisos.extend(saidas.tortos);
        if saidas.de_fabrica.is_empty() {
            return;
        }
        self.avisos.push(format!(
            "estas saidas vao PEDIR o aperto de mao da cifra do fio, e ninguem \
             escreveu essa decisao no arquivo: {}. Desde 18/09/2026 o \
             interruptor \"cifra\" de cada saida nasce LIGADO (ordem do dono: \
             a comunicacao deve obrigatoriamente ser cifrada) -- entao um \
             PhxSql anterior a essa data, do outro lado, NAO atende o aperto e \
             a conexao para. Escreva \"cifra\": true para registrar que o \
             outro lado fala o aperto, ou \"cifra\": false para voltar ao \
             claro; qualquer uma das duas cala este aviso. Receita em \
             docs/CIFRA-DO-FIO.md 8.",
            saidas.de_fabrica.join(", ")
        ));
    }

    /// Este servidor vai MESMO puxar de `replicacao.origens`?
    ///
    /// Com o bloco `cluster` a lista e IGNORADA -- quem puxa e um laco so, do
    /// master CORRENTE, descoberto pelo pulso --, e um papel que nao puxa nao
    /// sobe laco nenhum. As duas condicoes ja estavam escritas a mao na regra
    /// que exige a lista, logo abaixo; agora moram num lugar so.
    pub(crate) fn puxa_das_origens(&self) -> bool {
        self.cluster.is_none() && self.replicacao.papel.puxa_de_origem()
    }

    /// ...e com DUAS ou mais threads puxando em paralelo?
    ///
    /// E a pergunta unica que liga a guarda de sobreposicao de database
    /// (pedido 406) nos DOIS niveis -- o estatico
    /// ([`Config::recusar_origens_ambiguas`], aqui) e o dinamico
    /// (`Servidor::ligar_guarda_de_databases`). Ela mora aqui, e nao numa
    /// copia de cada lado, porque a copia que envelhecesse seria um nivel
    /// ligado onde o outro esta desligado: foi exatamente o que aconteceu no
    /// primeiro corte do 406 -- o dinamico se desligava com `cluster` e o
    /// estatico nao, e um no de cluster com origens sobrando de antes deixava
    /// de subir. Pior: `validar()` tambem roda no `gravar_a_arvore`, entao a
    /// tela de configuracao recusava gravar o MESMO arquivo que estava no
    /// disco, e o operador perdia as duas saidas no mesmo upgrade.
    pub(crate) fn puxa_de_varias_origens(&self) -> bool {
        self.puxa_das_origens() && self.replicacao.origens.len() >= 2
    }

    /// As tres recusas do nivel ESTATICO da guarda de sobreposicao de
    /// database (pedido 406): o que da para saber lendo o `config.json`,
    /// antes de qualquer conexao.
    ///
    /// O portao que decide vem ANTES do trabalho e e UM so -- as tres recusam
    /// arranjos que so existem quando ha duas threads puxando em paralelo, e
    /// espalhar a condicao por tres funcoes deixaria a que alguem esquecesse
    /// mordendo onde a guarda nao e para morder.
    fn recusar_origens_ambiguas(&self) -> Result<()> {
        if !self.puxa_de_varias_origens() {
            return Ok(());
        }
        // O nome primeiro: com ele repetido as outras duas mensagens nomeiam
        // duas origens que o operador nao consegue distinguir no arquivo.
        self.recusar_nome_de_origem_repetido()?;
        self.recusar_duas_origens_sem_databases()?;
        self.recusar_database_em_duas_origens()
    }

    /// Duas origens com o MESMO nome nao sobem.
    ///
    /// O nome e a identidade da conexao em `replicacao_estado`,
    /// `replicacao_pular`, `replicacao_ligar` e na guarda de database, e nas
    /// quatro a segunda se faz passar pela primeira. Na guarda e pior que
    /// confusao: `dono.origem == origem` da verdadeiro para as duas, e a
    /// guarda inteira se desliga justamente no arranjo que ela existe para
    /// pegar. E nao precisa de malicia nenhuma -- `"nome"` e opcional e o
    /// padrao e `"origem"` (ver `Config::de_json`), entao basta OMITIR um
    /// campo opcional duas vezes.
    ///
    /// A recusa nomeia posicao e `host:porta` de cada uma, e nao so o nome:
    /// o nome e justamente o que nao as distingue.
    fn recusar_nome_de_origem_repetido(&self) -> Result<()> {
        for (i, a) in self.replicacao.origens.iter().enumerate() {
            for (j, b) in self.replicacao.origens.iter().enumerate().skip(i + 1) {
                if a.nome != b.nome {
                    continue;
                }
                return Err(PhxError::Esquema(format!(
                    "replicacao.origens[{i}] ({}:{}) e [{j}] ({}:{}) tem o mesmo \
                     nome {:?}: o nome identifica a conexao no estado, no pular, \
                     no ligar e na guarda de database, e com ele repetido a \
                     segunda origem se faz passar pela primeira -- as duas se \
                     veem donas do mesmo database e a guarda nao morde. O campo \
                     \"nome\" e obrigatorio quando ha mais de uma origem -- ver \
                     docs/REPLICACAO.md",
                    a.host, a.porta, b.host, b.porta, a.nome
                )));
            }
        }
        Ok(())
    }

    /// Duas origens SEM `databases` nao sobem: o arranjo e indecidivel.
    ///
    /// Lista vazia quer dizer «todos os databases desta origem», e quais sao
    /// eles so a origem sabe. Com UMA vazia a descoberta decide sem sortear --
    /// as declaradas reivindicam seus nomes antes de qualquer laco subir, e
    /// entre uma escolha escrita e um curinga ganha a escolha. Com DUAS, nao
    /// ha escolha escrita nenhuma: o dono de um nome que as duas entreguem
    /// seria a thread que conectasse primeiro, e isso muda a cada arranque por
    /// latencia de rede. Antes da guarda as duas escreviam juntas e o
    /// fail-stop chegava cedo; sortear o vencedor trocaria uma falha cedo por
    /// uma falha intermitente, que e mais dificil de diagnosticar, nao mais
    /// facil. Por isso se recusa o arranjo em vez de sortear o vencedor.
    fn recusar_duas_origens_sem_databases(&self) -> Result<()> {
        let sem: Vec<&Origem> = self
            .replicacao
            .origens
            .iter()
            .filter(|o| o.databases.is_empty())
            .collect();
        if sem.len() < 2 {
            return Ok(());
        }
        Err(PhxError::Esquema(format!(
            "as origens {:?} e {:?} estao sem \"databases\" em \
             replicacao.origens: lista vazia quer dizer «todos os databases \
             desta origem», e com duas assim o dono de um nome que as duas \
             entreguem seria a thread que conectasse primeiro -- outro a cada \
             arranque. Declare \"databases\" em todas menos uma (no maximo UMA \
             origem pode ficar sem lista) -- ver docs/REPLICACAO.md",
            sem[0].nome, sem[1].nome
        )))
    }

    /// Duas origens declarando o MESMO database nao sobem.
    ///
    /// # Por que a recusa e aqui, no arranque
    ///
    /// As duas origens escreveriam no MESMO `.reg` daqui, e a replica fiel
    /// aplica por ROWID: no segundo evento da segunda origem os rowids
    /// divergem. A inclusao ainda fail-stopa (`table.rs`, a conferencia do
    /// rowid), mas a ALTERACAO nao tem conferencia nenhuma -- ela sobrescreve
    /// calada a linha da outra origem. Arranjo que se destroi sozinho tem de
    /// morrer no arranque, nomeando as duas origens e o database, e nao no
    /// meio do expediente. Parecer do papel C de 23/09/2026, secao 6.
    ///
    /// # Por que SO o cruzamento de listas declaradas
    ///
    /// Lista vazia quer dizer «todos os databases daquela origem», e quais
    /// sao eles so a origem sabe -- aqui seria palpite. Uma vazia contra uma
    /// declarada e **indecidivel neste ponto**: quem sabe e a descoberta, e e
    /// la que a recusa acontece, por database e sem derrubar os outros. (Duas
    /// vazias tambem sao indecidiveis, e por isso nao sao caso deste metodo:
    /// o arranjo inteiro morre em `recusar_duas_origens_sem_databases`, que a
    /// descoberta nao tinha como decidir sem sortear.) Recusar por palpite
    /// tambem tiraria do ar o
    /// `Config_exemplo_03.json`, que traz curitiba `["Z"]`, saopaulo (vazia) e
    /// bruxelas `["W"]` e e arranjo legitimo -- e um par 1<->1 tem uma origem
    /// so, que nao cruza com ninguem. Guarda nova entra pedida, nao imposta.
    fn recusar_database_em_duas_origens(&self) -> Result<()> {
        for (i, a) in self.replicacao.origens.iter().enumerate() {
            for b in self.replicacao.origens.iter().skip(i + 1) {
                let Some(db) = a.databases.iter().find(|d| b.databases.contains(d)) else {
                    continue;
                };
                return Err(PhxError::Esquema(format!(
                    "as origens {:?} e {:?} declaram o mesmo database {db:?} em \
                     replicacao.origens[].databases: as duas escreveriam no \
                     mesmo {db} deste servidor, os rowids divergem e a \
                     replicacao para. Cada origem tem de entregar um nome de \
                     database so dela -- ver docs/REPLICACAO.md",
                    a.nome, b.nome
                )));
            }
        }
        Ok(())
    }

    // `pub(crate)` para que o teste do cluster em `servidor.rs` possa chama-lo:
    // a guarda de database tem dois niveis em dois arquivos, e um teste que so
    // alcanca metade dela foi exatamente o que deixou passar a regressao do
    // primeiro corte do pedido 406.
    pub(crate) fn validar(&self) -> Result<()> {
        if self.token.trim().is_empty() {
            return Err(PhxError::Esquema(
                "config.json sem token: preencha o campo \"token\" antes de subir o servidor"
                    .into(),
            ));
        }
        self.endereco()?;
        if self.web.ligado {
            let web = self.web.endereco()?;
            if enderecos_colidem(web, self.endereco()?) {
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
        // As duas portas do REST entram na MESMA conta das outras: a colisao
        // descoberta no arranque custa uma mensagem, e a descoberta depois
        // custa um ouvinte calado que ninguem sabe que nao subiu.
        for (rotulo, ligado, alvo) in [
            ("rest.bind", self.rest.ligado, self.rest.endereco()),
            (
                "rest.swagger_bind",
                self.rest.swagger_ligado,
                self.rest.endereco_do_swagger(),
            ),
        ] {
            if !ligado {
                continue;
            }
            let alvo = alvo?;
            if let Some((quem, _)) = ocupadas.iter().find(|(_, e)| enderecos_colidem(*e, alvo)) {
                return Err(PhxError::Esquema(format!(
                    "{rotulo} e {quem} apontam para o mesmo endereco ({alvo})"
                )));
            }
            ocupadas.push((rotulo, alvo));
        }
        for (rotulo, texto) in self.replicacao.portas() {
            let alvo = Replicacao::resolver(rotulo, texto)?;
            if let Some((quem, _)) = ocupadas.iter().find(|(_, e)| enderecos_colidem(*e, alvo)) {
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
        if self.puxa_das_origens() && self.replicacao.origens.is_empty() {
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
        self.recusar_origens_ambiguas()?;
        if let Some(c) = &self.cluster {
            c.validar(&self.replicacao)?;
        }
        // O pino de cada servidor da interface e conferido no arranque, pela
        // mesma razao do pino das origens: hexadecimal torto tem de virar erro
        // NA HORA em que se escreveu, e nao um tunel sem pino descoberto no dia
        // do primeiro `Remoto`. `pino_do_fio` ja carrega essa disciplina.
        for s in &self.web.servidores {
            s.pino_do_fio()?;
        }
        // A lista de tabelas sigilosas e conferida SEMPRE, e nao so com a
        // cifra ligada: quem escreve a lista antes de ligar a cifra -- que e a
        // ordem natural de quem esta configurando -- merece o erro na hora em
        // que escreveu, e nao meses depois.
        self.cifra.validar()?;
        // A chave mestra do DbLink escrita no lugar errado recusa o arranque:
        // e DECLARACAO torta, e nao chave ausente. A ausente so tranca as
        // ligacoes cifradas; esta anunciaria uma protecao que nao existe.
        self.cifra_do_dblink.validar()?;
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
    ///
    /// Delega ao `hash.rs`, que e onde a lei mora: este metodo tinha o laco
    /// escrito de novo aqui, e o irmao do REST (`Rest::token_confere`) nao
    /// tinha laco nenhum -- comparava com `==`. Uma implementacao, dois
    /// chamadores.
    pub fn token_confere(&self, oferecido: &str) -> bool {
        phxsql_core::hash::iguais_em_tempo_constante(self.token.as_bytes(), oferecido.as_bytes())
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
                                        // O estado do FIO, pela mesma forma
                                        // que `web.servidores` e os nos do
                                        // cluster ja publicam: `cifra` e o
                                        // interruptor, `tem_pino` diz se ha
                                        // ancora SEM carregar material de
                                        // chave -- o pino nunca sai numa
                                        // resposta de protocolo. Sem estes
                                        // dois a tela mandava colar um bloco
                                        // cifrado e nao tinha como CONFERIR
                                        // que ele valeu, que e o passo que o
                                        // proprio assistente oferece.
                                        ("cifra", Json::Bool(o.cifra)),
                                        ("tem_pino", Json::Bool(!o.chave_do_fio.is_empty())),
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
                    (
                        "contar_injecao_sql",
                        Json::Bool(self.politica.contar_injecao_sql),
                    ),
                    (
                        "contar_linha_acima_do_teto",
                        Json::Bool(self.politica.contar_linha_acima_do_teto),
                    ),
                    (
                        "contar_pulso_desconhecido",
                        Json::Bool(self.politica.contar_pulso_desconhecido),
                    ),
                ]),
            ),
            (
                "web",
                Json::objeto(vec![
                    ("ligado", Json::Bool(self.web.ligado)),
                    ("bind", Json::texto_de(&self.web.bind)),
                    ("sessao_minutos", Json::de_u64(self.web.sessao_minutos)),
                    ("atras_de_proxy", Json::Bool(self.web.atras_de_proxy)),
                    (
                        // So o endereco, como sempre foi -- a tela de
                        // Configuracoes junta esta lista num texto. O estado do
                        // tunel (cifra e se ha pino) sai por outro caminho, o
                        // `/saude`, que e onde o login escolhe o destino.
                        "servidores",
                        Json::Lista(
                            self.web
                                .servidores
                                .iter()
                                .map(|s| Json::texto_de(&s.endereco))
                                .collect(),
                        ),
                    ),
                ]),
            ),
            ("rest", self.rest.para_json()),
            // O bloco do CLUSTER, que faltava inteiro -- pedido 218.
            //
            // O `Cluster::para_json` existia desde que o cluster nasceu, e
            // NINGUEM o chamava: a tela pedia `config`, a chave nao vinha, e
            // uma tela de cluster nao tinha de onde ler. Ausencia e pior que
            // ocultacao -- `seguranca` e `cifra` aparecem com o segredo
            // trocado por um rotulo, e quem le sabe que existem; `cluster`
            // nao aparecia de jeito nenhum, e quem lia concluia que nao havia
            // cluster nenhum configurado.
            //
            // `Nulo` quando nao ha bloco, e isso e o oposto de omitir: a tela
            // distingue "este servidor nao esta em cluster" de "o servidor
            // nao me contou". O token e o `senha_hash` continuam de fora, por
            // dentro do proprio `Cluster::para_json` -- resposta de protocolo
            // nao carrega credencial.
            (
                "cluster",
                match &self.cluster {
                    Some(c) => c.para_json(),
                    None => Json::Nulo,
                },
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
            ("cifra_do_dblink", self.cifra_do_dblink.para_json()),
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
            ("acessos", self.acessos.para_json()),
            ("diretivas", self.diretivas.para_json()),
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
    /// Lista de textos -- a `rest.tabelas`, hoje.
    ///
    /// Tipo proprio, e nao `Texto` com virgula dentro, porque o que a tela
    /// grava tem de ser o que o leitor le: `textos()` quer uma lista JSON, e
    /// uma string com virgulas viraria UMA tabela chamada "a, b". O recorte
    /// acontece na tela, que e onde a pessoa digita.
    Lista,
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
            TipoDoCampo::Lista => v
                .lista()
                .is_some_and(|l| l.iter().all(|x| x.texto().is_some())),
            TipoDoCampo::Cor => v.texto().is_some_and(cor_valida),
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            TipoDoCampo::Inteiro => "inteiro",
            TipoDoCampo::Numero => "numero",
            TipoDoCampo::Booleano => "booleano",
            TipoDoCampo::Texto => "texto",
            TipoDoCampo::Lista => "lista",
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
    // Os dois da web nao valem a quente pelo mesmo motivo do `conexoes_max`:
    // o semaforo nasce com o teto no arranque, junto do laco de aceitacao.
    ("recursos.conexoes_web_max", TipoDoCampo::Inteiro, false),
    ("recursos.fila_web_ms", TipoDoCampo::Inteiro, false),
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
    // O webservice REST. Tudo aqui vale NO ARRANQUE, e a tela diz isso ao
    // lado: abrir e fechar porta de rede a quente seria dar, a quem tomasse
    // uma sessao de administrador, o poder de expor o servidor sem reiniciar
    // nada -- e o `false` e o que faz a tela prometer o que o servidor cumpre.
    //
    // `rest.token` NAO esta aqui, e a ausencia e a mesma decisao que ja
    // manteve `token` de fora: campo que carrega credencial se edita no
    // arquivo. A tela mostra se existe um, nunca qual e.
    ("rest.ligado", TipoDoCampo::Booleano, false),
    ("rest.bind", TipoDoCampo::Texto, false),
    ("rest.nome", TipoDoCampo::Texto, false),
    ("rest.database", TipoDoCampo::Texto, false),
    ("rest.tabelas", TipoDoCampo::Lista, false),
    ("rest.swagger_ligado", TipoDoCampo::Booleano, false),
    ("rest.swagger_bind", TipoDoCampo::Texto, false),
    // A lista de tabelas sigilosas vale A QUENTE, e e a unica coisa da secao
    // `cifra` que se edita pela tela: a senha e as iteracoes ficam no arquivo,
    // porque campo que carrega credencial nao se digita numa tela que qualquer
    // administrador abre. A lista nao carrega segredo -- so nomes de tabela --
    // e quem a muda quer o efeito AGORA, no Profiler que ja esta ligado.
    ("cifra.tabelas", TipoDoCampo::Lista, true),
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
    // Pedido 228: o mesmo rodizio do Profiler, para `acessos.log` e
    // `diretivas.log`. `true` (vale a quente) pelo mesmo motivo: quem baixou
    // o teto na tela quer o efeito no arquivo CORRENTE, e `gravar_campos`
    // leva os dois ao `LogAcessos`/`Diario` vivos em `op_config_gravar`.
    ("acessos.arquivo_mib", TipoDoCampo::Inteiro, true),
    ("acessos.arquivos", TipoDoCampo::Inteiro, true),
    ("diretivas.arquivo_mib", TipoDoCampo::Inteiro, true),
    ("diretivas.arquivos", TipoDoCampo::Inteiro, true),
    ("telemetria.alto_uso_ms", TipoDoCampo::Inteiro, true),
    ("telemetria.stress_ms", TipoDoCampo::Inteiro, true),
    // O quorum minimo da escrita -- pedido 208, a tela; pedido 207, o efeito.
    //
    // `false` (nao vale a quente) e a marca HONESTA hoje: nada o le, entao
    // gravar nao muda comportamento nenhum. A tela mostra o gravado pelo
    // caminho que ja existe (`no_arquivo`) e diz ao lado que o campo ainda
    // nao e imposto. Quando o 207 entrar, esta linha vira `true` no mesmo
    // commit que fizer o commit espera-lo.
    //
    // Os OUTROS campos do bloco `cluster` continuam fora: `nos` tem operacao
    // propria (`cluster_no_acrescentar`), e `token`/`usuario`/`senha_hash`
    // carregam credencial -- e credencial se edita no arquivo.
    ("cluster.quorum_minimo", TipoDoCampo::Inteiro, false),
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
    let Ok(texto) = crate::config_phz::ler_texto(caminho) else {
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
        Self::gravar_arvore(caminho, mudancas)
    }

    /// O ESCRITOR: troca os campos no arquivo, valida e grava.
    ///
    /// # Por que ele e separado do porteiro
    ///
    /// A lista de nos do cluster tambem se grava no `config.json` (pedido
    /// 217, escalonar a quente), e ela NAO passa por `CAMPOS_EDITAVEIS`: o
    /// formulario generico da tela nao a monta, e o portao dela e outro --
    /// mexer no denominador da maioria e decisao de cluster, nao de campo de
    /// configuracao. Duas gravacoes com duas escritas divergiriam no primeiro
    /// cuidado que uma ganhasse (a permissao herdada, o `rename` atomico, o
    /// cinto que confere o texto contra a arvore). Entao ha UM escritor, e
    /// dois porteiros na frente dele.
    fn gravar_arvore(caminho: &Path, mudancas: &[(String, Json)]) -> Result<Config> {
        let texto = crate::config_phz::ler_texto(caminho)?;
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

        let caminhos: Vec<Vec<String>> = mudancas
            .iter()
            .map(|(campo, _)| campo.split('.').map(str::to_string).collect())
            .collect();
        gravar_a_arvore(caminho, &texto, arvore, &caminhos)
    }

    /// Le o `config.json`, deixa `mexer` alterar a arvore e grava.
    ///
    /// # Por que ela existe ao lado do `gravar_campos`, e nao dentro dele
    ///
    /// Porque `usuarios` esta FORA de [`CAMPOS_EDITAVEIS`] de proposito, e
    /// continua: a tela de configuracao nao grava o cadastro, e uma sessao
    /// roubada nao cria supervisor mandando um campo a mais no formulario.
    /// O que muda com o pedido 221 e que ha uma porta PROPRIA para o cadastro
    /// -- com portao proprio, guardas proprias e a senha virando hash antes de
    /// tocar na arvore --, e ela usa o MESMO caminho de gravacao: mesma
    /// validacao antes, mesma troca cirurgica no texto, mesmo temporario
    /// 0600 desde o primeiro byte, mesmo `rename`.
    ///
    /// Duas gravacoes de `config.json` com caminhos diferentes seriam dois
    /// jeitos de o arquivo ficar pela metade, e so um deles estaria provado.
    pub fn gravar_a_secao(
        caminho: &Path,
        secao: &str,
        mexer: impl FnOnce(&mut Json) -> Result<()>,
    ) -> Result<Config> {
        let texto = crate::config_phz::ler_texto(caminho)?;
        let mut arvore = Json::analisar(&texto)?;
        mexer(&mut arvore)?;
        gravar_a_arvore(caminho, &texto, arvore, &[vec![secao.to_string()]])
    }

    /// Grava a lista de nos do cluster no `config.json` -- pedido 217.
    ///
    /// # Por que ela precisa ir ao arquivo, e nao so a memoria
    ///
    /// A lista viva do `EstadoCluster` faz o no NOVO ser aceito agora; sem
    /// gravar, o proximo arranque leria o arquivo velho e o no acrescentado
    /// sumiria calado -- que e a pior forma de perder um no, porque o cluster
    /// continua funcionando com um denominador de maioria menor do que o
    /// operador acredita.
    ///
    /// A validacao e a mesma de sempre (`Config::validar`), entao lista com
    /// id repetido, sem este servidor dentro ou com menos de dois nos volta
    /// recusada ANTES de tocar o disco.
    pub fn gravar_nos_do_cluster(caminho: &Path, nos: &[NoCluster]) -> Result<Config> {
        let lista = Json::Lista(
            nos.iter()
                .map(|n| {
                    let mut campos = vec![
                        ("id", Json::texto_de(&n.id)),
                        ("endereco", Json::texto_de(&n.endereco)),
                        ("porta", Json::de_u64(n.porta as u64)),
                    ];
                    // O pino sobrevive ao reescrever a lista inteira: sem esta
                    // linha, um `cluster_no_acrescentar`/`_remover` a quente
                    // reescreveria `cluster.nos` sem os pinos e deixaria TODO
                    // no sem ancora no proximo arranque -- a cifra do cluster
                    // continuaria ligada, so que rebaixada a escuta passiva em
                    // silencio. So sai quando existe: cluster em claro nao
                    // ganha campo vazio nenhum no arquivo.
                    if !n.chave_do_fio.is_empty() {
                        campos.push(("chave_do_fio", Json::texto_de(&n.chave_do_fio)));
                    }
                    Json::objeto(campos)
                })
                .collect(),
        );
        Self::gravar_arvore(caminho, &[("cluster.nos".to_string(), lista)])
    }

    pub fn acrescentar_proibidos_da_base(
        caminho: &Path,
        base: &str,
        comandos: &[String],
    ) -> Result<(Config, Vec<String>, Vec<String>)> {
        let base = base.trim();
        if base.is_empty() {
            return Err(PhxError::Esquema(
                "informe o banco: ALTER DATABASE <banco> SET comandos_proibidos = (…)".into(),
            ));
        }
        let pedidos: Vec<String> = comandos
            .iter()
            .map(|c| c.trim().to_lowercase())
            .filter(|c| !c.is_empty())
            .collect();
        if pedidos.is_empty() {
            return Err(PhxError::Esquema(
                "esta diretiva so ACRESCENTA: mande ao menos um comando. Para \
                 RETIRAR um da lista, edite \"seguranca.comandos_proibidos\" no \
                 config.json -- politica que a rede afrouxa nao e politica"
                    .into(),
            ));
        }

        let texto = crate::config_phz::ler_texto(caminho)?;
        let mut arvore = Json::analisar(&texto)?;
        let seguranca = match arvore.campo("seguranca") {
            None => Json::Objeto(Vec::new()),
            Some(Json::Objeto(pares)) => Json::Objeto(pares.clone()),
            Some(outro) => {
                return Err(PhxError::Esquema(format!(
                    "\"seguranca\" no arquivo nao e um objeto: {}",
                    outro.escrever()
                )))
            }
        };
        let mut lista = seguranca
            .campo("comandos_proibidos")
            .and_then(Json::lista)
            .map(<[Json]>::to_vec)
            .unwrap_or_default();

        // O que ja esta la, na forma por banco. A comparacao e sobre o par
        // (banco, comando), e nao sobre o texto da entrada: a mesma proibicao
        // escrita com as chaves em outra ordem e a mesma proibicao.
        let ja: Vec<String> = crate::blacklist::proibidos_por_base(&seguranca)
            .into_iter()
            .filter(|(b, _)| b == base)
            .map(|(_, c)| c)
            .collect();
        // E o que o GLOBAL ja proibe: repetir por banco o que ja vale para
        // todos nao acrescenta guarda nenhuma, e enche a lista de entradas que
        // um dia divergem da global.
        let globais = crate::blacklist::proibidos_globais(&seguranca);

        let mut entraram = Vec::new();
        let mut existiam = Vec::new();
        for c in pedidos {
            if ja.contains(&c) || globais.contains(&c) {
                existiam.push(c);
                continue;
            }
            lista.push(Json::objeto(vec![
                ("comando", Json::texto_de(&c)),
                ("database", Json::texto_de(base)),
            ]));
            entraram.push(c);
        }

        let mut seguranca = seguranca;
        seguranca.definir("comandos_proibidos", Json::Lista(lista));
        arvore.definir("seguranca", seguranca);
        // Religado ao escritor livre `gravar_a_arvore` (o mesmo que a
        // gravacao por campo usa) na integracao: o F3 trazia um escritor
        // proprio de mesma assinatura, que duplicaria o da F5. Um escritor so.
        let novo = gravar_a_arvore(
            caminho,
            &texto,
            arvore,
            &[vec![
                "seguranca".to_string(),
                "comandos_proibidos".to_string(),
            ]],
        )?;
        Ok((novo, entraram, existiam))
    }
}

/// Valida a arvore e a grava atomicamente, trocando `caminhos` NO TEXTO.
///
/// # A validacao vem ANTES da gravacao
///
/// A arvore alterada passa por `de_json` + `validar` primeiro: valor que nao
/// subiria o servidor nao entra no arquivo.
fn gravar_a_arvore(
    caminho: &Path,
    texto: &str,
    arvore: Json,
    caminhos: &[Vec<String>],
) -> Result<Config> {
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
    let mut corpo = texto.to_string();
    for partes in caminhos {
        let partes: Vec<&str> = partes.iter().map(String::as_str).collect();
        let Some(valor) = valor_em(&arvore, &partes.join(".")) else {
            corpo = arvore.escrever_identado();
            corpo.push('\n');
            break;
        };
        // Objeto e lista GRANDES vao identados; o resto vai compacto.
        //
        // A escolha e por FORMA e TAMANHO do valor, e nao por nome de campo:
        // uma lista de duas tabelas cabe numa linha e fica melhor assim, mas
        // a `usuarios` inteira compacta vira uma linha de dois mil caracteres
        // com o cadastro dentro -- e o cadastro e justamente a secao que
        // gente le e edita a mao. O corte fica no tamanho, porque e o tamanho
        // que decide se a linha se le.
        let cabe_numa_linha =
            !matches!(valor, Json::Objeto(_) | Json::Lista(_)) || valor.escrever().len() <= 120;
        let trocado = if cabe_numa_linha {
            Json::texto_trocar(&corpo, &partes, &valor)
        } else {
            Json::texto_trocar_identado(&corpo, &partes, &valor)
        };
        match trocado {
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

    // O arquivo carrega o token e os hashes: nasce 0600 desde o primeiro
    // byte e entra por troca atomica -- o mesmo molde da chave do fio e do
    // cadastro do DbLink, num lugar so. Ele NAO herda mais a permissao do
    // original: um `0644` de instalacao virava `0644` para sempre. A FORMA
    // (claro ou `.phz`, pedido 450) sai da extensao do arquivo que o servidor
    // leu, e se decide no mesmo motor que o le.
    crate::config_phz::gravar_texto(caminho, &corpo)?;
    Ok(novo)
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
        assert!(c.cifra.senha().unwrap().is_empty());
        assert!(c.estranhas.is_empty());
        // E aplicar uma cifra desligada nao liga cofre nenhum.
        c.cifra.aplicar().unwrap();
        assert!(!phxsql_store::cofre::ligado());
    }

    /// **O padrao, e ele MUDOU DE SIGNIFICADO em 18/09/2026 (pedido 370).**
    ///
    /// Ate ontem este teste se chamava `sem_a_secao_cifra_fio_nada_e_exigido` e
    /// travava o contrario: sem a secao, nada era exigido, porque guarda nova
    /// entra PEDIDA. Quem revogou isso foi o dono, em palavra propria -- *«A
    /// comunicacao deve obrigatoriamente ser cifrada»* --, e o teste nao foi
    /// apagado: **ele mudou de lado, e o lado novo esta escrito aqui.** Teste
    /// que some leva a garantia junto, e a garantia continua sendo a mesma
    /// pergunta: «o que acontece com quem so trocou o binario?»
    ///
    /// A resposta nova, dita sem enfeite: **o cliente que nao fala o aperto
    /// para de entrar pela porta de dados, e a porta HTTP sem proxy declarado
    /// para de atender**, ate alguem escrever `"exigir": false` ou
    /// `"atras_de_proxy": true`. Isso e mudanca de implantacao, foi decidida, e
    /// e o que a linha abaixo trava contra um retorno silencioso.
    ///
    /// O que NAO mudou, e por isso continua travado aqui: `exigir_amarra`
    /// nasce desligada (guarda nova entra pedida continua valendo para ela) e
    /// `ligada` nasce ligada, que nunca mudou nada para ninguem.
    #[test]
    fn sem_a_secao_cifra_fio_a_cifra_ja_e_exigida() {
        let j = Json::analisar(r#"{"token":"t"}"#).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(
            c.cifra_fio.exigir,
            "sem a secao, o servidor NAO exige o tunel: a ordem do dono de \
             18/09/2026 voltou atras sem ninguem ter pedido"
        );
        assert!(
            !c.cifra_fio.exigir_amarra,
            "sem a secao, o servidor passou a EXIGIR a amarracao do canal: \
             quem so pede o tunel para de entrar"
        );
        // `ligada` NASCE ligada, e isso nunca mudou nada para ninguem: o aperto
        // so acontece se o cliente pedir.
        assert!(c.cifra_fio.ligada);
        assert!(c.estranhas.is_empty());
    }

    #[test]
    fn a_secao_cifra_fio_e_lida_e_nao_vira_campo_estranho() {
        let j = Json::analisar(
            r#"{"token":"t","cifra_fio":{"ligada":false,"exigir":true,
                 "exigir_amarra":true,"arquivo":"/tmp/uma-chave.hex"}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(!c.cifra_fio.ligada);
        assert!(c.cifra_fio.exigir);
        assert!(c.cifra_fio.exigir_amarra);
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

    /// NENHUM segredo do `config.json` sai no `Debug` -- nem o do topo, nem o
    /// das pecas de dentro.
    ///
    /// Irma da `a_privada_do_fio_nunca_sai`, e do mesmo defeito: ali a
    /// [`CifraFio`] ja escrevia o proprio `Debug`, e a [`Cifra`] tambem. As
    /// OUTRAS seis derivavam, entao um `dbg!(&config)` num diagnostico
    /// apressado despejava o token do protocolo, o do cluster, o do REST, a
    /// senha do rele de e-mail e as credenciais de cada origem de replicacao.
    ///
    /// A prova monta UM `Config` com todos eles porque e assim que o vazamento
    /// acontece de verdade: ninguem imprime uma `Origem` solta, imprime o
    /// `Config` inteiro.
    ///
    /// Confere as duas formas (`{:?}` e `{c:?}`) pelo mesmo motivo da irma.
    #[test]
    fn nenhum_segredo_do_config_sai_no_debug() {
        // Cada segredo com um valor DIFERENTE e improvavel: se dois fossem
        // iguais, um vazamento se esconderia atras do outro.
        const TOKEN: &str = "token-do-protocolo-daqui";
        const TOKEN_CLUSTER: &str = "token-do-cluster-daqui";
        const TOKEN_REST: &str = "token-da-porta-rest";
        const TOKEN_ORIGEM: &str = "token-do-source-remoto";
        const SENHA_EMAIL: &str = "senha-do-rele-de-email";
        const SENHA_ORIGEM: &str = "senha-em-claro-da-origem";
        const HASH_ORIGEM: &str = "hash-da-senha-da-origem";
        const HASH_CLUSTER: &str = "hash-da-senha-do-cluster";
        // O pino e chave PUBLICA: tem de continuar aparecendo.
        const PINO: &str = "aabbccddeeff00112233445566778899";

        let j = Json::analisar(&format!(
            r#"{{"token":"{TOKEN}",
                 "alertas":{{"email":{{"senha":"{SENHA_EMAIL}"}}}},
                 "rest":{{"token":"{TOKEN_REST}"}},
                 "replicacao":{{"papel":"replica","origens":[
                     {{"nome":"matriz","token":"{TOKEN_ORIGEM}",
                       "senha":"{SENHA_ORIGEM}","senha_hash":"{HASH_ORIGEM}",
                       "chave_do_fio":"{PINO}"}}]}},
                 "cluster":{{"id":"a","token":"{TOKEN_CLUSTER}",
                     "senha_hash":"{HASH_CLUSTER}",
                     "nos":[{{"id":"a","endereco":"127.0.0.1"}}]}}}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).unwrap();

        // Os valores estao mesmo la -- senao a prova passaria por nao haver
        // segredo nenhum para vazar.
        assert_eq!(c.token, TOKEN);
        assert_eq!(c.alertas.email.senha().unwrap(), SENHA_EMAIL);
        assert_eq!(c.rest.token, TOKEN_REST);
        assert_eq!(c.replicacao.origens[0].token, TOKEN_ORIGEM);
        assert_eq!(c.replicacao.origens[0].senha, SENHA_ORIGEM);
        assert_eq!(c.replicacao.origens[0].senha_hash, HASH_ORIGEM);
        let cl = c.cluster.as_ref().expect("o cluster nao foi lido");
        assert_eq!(cl.token, TOKEN_CLUSTER);
        assert_eq!(cl.senha_hash, HASH_CLUSTER);

        for texto in [format!("{:?}", c), format!("{c:?}")] {
            for (que, segredo) in [
                ("o token do protocolo", TOKEN),
                ("o token do cluster", TOKEN_CLUSTER),
                ("o token do REST", TOKEN_REST),
                ("o token da origem", TOKEN_ORIGEM),
                ("a senha do e-mail", SENHA_EMAIL),
                ("a senha da origem", SENHA_ORIGEM),
                ("o hash da origem", HASH_ORIGEM),
                ("o hash do cluster", HASH_CLUSTER),
            ] {
                assert!(
                    !texto.contains(segredo),
                    "{que} vazou no Debug do Config: {texto}"
                );
            }
            // O pino e publico e continua visivel: `Debug` cego nao
            // diagnostica pino torto.
            assert!(texto.contains(PINO), "o pino sumiu do Debug: {texto}");
            // E o resto do Config continua legivel.
            assert!(texto.contains("matriz"), "o nome da origem sumiu: {texto}");
        }
    }

    /// A estatica nasce no arquivo, ao lado do `config.json`, e a SEGUNDA
    /// leitura devolve a mesma -- senao o pino de todo cliente quebraria a
    /// cada arranque.
    #[test]
    fn a_estatica_do_fio_nasce_no_arquivo_e_nao_muda() {
        let d = DirTemp::novo("chave-do-fio");
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

    /* ------------------------------- a senha do banco nao e a de administracao
    Palavra do dono, 05/09/2026. A regra e do desenho de senha por banco -- que
    ainda nao existe --, mas a CONFERENCIA existe hoje e vale para o que ha:
    `cifra.senha`. */

    /// **O teste que mais importa: proibicao nova nao derruba servidor que ja
    /// existe.**
    ///
    /// Guarda nova entra PEDIDA, nao imposta. Um `config.json` que hoje tem a
    /// senha da cifra igual a de um administrador continua subindo -- ele so
    /// passa a AVISAR. Recusar aqui poria um banco no chao para consertar uma
    /// senha, e o estrago seria maior que o furo.
    #[test]
    fn proibicao_nova_nao_derruba_servidor_que_ja_existe() {
        let hash = phxsql_core::senha::cifrar_com("a mesma senha dos dois", 1_000);
        let j = Json::analisar(&format!(
            r#"{{"token":"t","cifra":{{"ligada":true,"senha":"a mesma senha dos dois"}},
                 "usuarios":[{{"login":"zoroastro","senha_hash":"{hash}","supervisor":true}}]}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).expect("o servidor tem de continuar subindo");
        c.validar().expect("e tem de continuar validando");
        assert!(
            c.avisos.iter().any(|a| a.contains("cifra.senha")),
            "subiu calado: quem tem as duas senhas iguais nunca vai saber -- {:?}",
            c.avisos
        );
        // E a mensagem NAO nomeia o administrador: ela ja e um oraculo pequeno
        // por natureza, e dizer QUEM colidiu o tornaria util.
        //
        // O login e `zoroastro` e nao `adm` de proposito: a primeira versao
        // deste teste procurava `"adm"` e reprovava sozinha, porque a palavra
        // «administrador» esta DENTRO da mensagem. Um casador que acha o que
        // procura no proprio texto da explicacao nao prova nada sobre o nome.
        assert!(
            !c.avisos.iter().any(|a| a.contains("zoroastro")),
            "o aviso nomeou o administrador: {:?}",
            c.avisos
        );
    }

    /// Senhas diferentes: nenhum aviso. E o caso comum, e ele nao pode ganhar
    /// ruido -- aviso que aparece sempre e aviso que ninguem le.
    #[test]
    fn senha_de_cifra_diferente_da_de_administrador_nao_avisa_nada() {
        let hash = phxsql_core::senha::cifrar_com("a senha do administrador", 1_000);
        let j = Json::analisar(&format!(
            r#"{{"token":"t","cifra":{{"ligada":true,"senha":"outra senha, a do banco"}},
                 "usuarios":[{{"login":"adm","senha_hash":"{hash}","supervisor":true}}]}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
    }

    /// A conferencia responde sem nunca ter a senha do administrador: ela
    /// deriva a candidata com o SAL e as ITERACOES do proprio hash guardado.
    ///
    /// E ela so olha SUPERVISOR: a regra do dono e sobre quem *administra*, e
    /// esticar a proibicao a todo usuario faria a senha do banco colidir com a
    /// de qualquer operador -- o que nao protege mais e proibe muito.
    #[test]
    fn a_conferencia_deriva_do_hash_e_so_olha_administrador() {
        let hash = phxsql_core::senha::cifrar_com("a senha do operador", 1_000);
        let ficha = |supervisor: bool| {
            let j = Json::analisar(&format!(
                r#"{{"token":"t","usuarios":[{{"login":"operador",
                     "senha_hash":"{hash}","supervisor":{supervisor}}}]}}"#
            ))
            .unwrap();
            crate::usuarios::Cadastro::de_json(&j).unwrap()
        };
        let cadastro = ficha(false);
        assert!(
            !Cifra::senha_e_de_algum_administrador("a senha do operador", &cadastro),
            "proibiu por causa de um usuario que nao administra nada"
        );
        // O mesmo usuario, agora supervisor: a mesma senha passa a colidir.
        let cadastro = ficha(true);
        assert!(
            Cifra::senha_e_de_algum_administrador("a senha do operador", &cadastro),
            "nao viu a colisao com um administrador"
        );
        // Senha vazia nunca colide -- e o estado de quem nao ligou a cifra.
        assert!(!Cifra::senha_e_de_algum_administrador("", &cadastro));
    }

    #[test]
    fn a_secao_cifra_e_lida_e_nao_vira_campo_estranho() {
        let j = Json::analisar(
            r#"{"token":"t","cifra":{"ligada":true,"senha":"abre-te sesamo","iteracoes":300000}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.cifra.ligada);
        assert_eq!(c.cifra.senha().unwrap(), "abre-te sesamo");
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
        assert_eq!(c.cifra.senha().unwrap(), "MARCA-SENHA-CIFRA");
        assert_eq!(c.alertas.email.senha().unwrap(), "MARCA-SENHA-RELE");
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

    /// Pedido 218: o bloco `cluster` tem de APARECER na resposta de `config`.
    ///
    /// # Por que este teste nasceu depois do outro que "ja cobria"
    ///
    /// O `nenhuma_credencial_do_config_sai_pela_op_config` acima ja conferia
    /// o token e o hash do cluster -- e passava com o bloco INTEIRO ausente,
    /// porque marca que nao foi serializada nao aparece. Guarda de vazamento
    /// so vale enquanto o campo existe; a que prova que ele existe e esta.
    #[test]
    fn o_bloco_cluster_aparece_na_op_config() {
        let bruto = r#"{"token":"t","replicacao":{"papel":"source","id_servidor":"n1",
            "imagem_da_linha":true},
          "cluster":{"id":"n1","prioridade":7,"janela_inatividade_s":9,
            "nos":[{"id":"n1","endereco":"10.0.0.1","porta":5001},
                   {"id":"n2","endereco":"10.0.0.2","porta":5002}]}}"#;
        let c = Config::de_json(&Json::analisar(bruto).unwrap()).unwrap();
        let j = c.para_json();
        let cl = j
            .campo("cluster")
            .expect("a chave \"cluster\" tem de existir");
        assert_eq!(cl.texto_ou("id", ""), "n1");
        assert_eq!(cl.inteiro_ou("prioridade", 0), 7);
        assert_eq!(cl.inteiro_ou("janela_inatividade_s", 0), 9);
        let nos = cl.campo("nos").and_then(Json::lista).expect("nos");
        assert_eq!(nos.len(), 2);
        assert_eq!(nos[1].texto_ou("id", ""), "n2");
        assert_eq!(nos[1].texto_ou("endereco", ""), "10.0.0.2");
        assert_eq!(nos[1].inteiro_ou("porta", 0), 5002);
    }

    /// Sem cluster a chave vem NULA, e nao ausente: a tela precisa distinguir
    /// "este servidor nao esta em cluster" de "o servidor nao me contou" --
    /// que era exatamente o buraco do 218.
    #[test]
    fn sem_cluster_a_chave_vem_nula_e_nao_some() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"t"}"#).unwrap()).unwrap();
        let j = c.para_json();
        assert!(
            matches!(j.campo("cluster"), Some(Json::Nulo)),
            "a chave tem de existir e ser nula: {}",
            j.escrever()
        );
    }

    // ---------------------------------------------------------------------
    // Pedido 372: a variavel declarada que falta. Nomes UNICOS por teste, e
    // as de falta nunca sao tocadas -- `set_var` e global e o `libtest` roda
    // em paralelo (pedidos 247, 261, 267).
    // ---------------------------------------------------------------------

    /// `cifra.senha_env` que falta: o cofre recusa NOMEANDO a variavel.
    ///
    /// O defeito aqui nao era calar -- era mentir: a senha vazia chegava ao
    /// cofre, e ele respondia «preencha cifra.senha ou cifra.senha_env» a quem
    /// tinha preenchido `senha_env`. Com o defeito reposto, `senha()` volta
    /// `Ok("")` e a primeira assercao cai.
    #[test]
    fn a_senha_da_cifra_que_falta_no_ambiente_e_erro_nomeado() {
        const AUSENTE: &str = "PHXSQL_TESTE_372_CIFRA_QUE_NINGUEM_EXPORTA";
        let j = Json::analisar(&format!(
            r#"{{"token":"t","cifra":{{"ligada":true,"senha_env":"{AUSENTE}"}}}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).expect("a leitura NAO pode recusar pela falta");
        // `match`, e nao `expect_err`: com o defeito reposto o `Ok` traria
        // o valor, e o `expect_err` o imprimiria na saida do teste.
        let e = match c.cifra.senha() {
            Ok(_) => panic!("variavel ausente virou senha vazia -- o defeito do 372"),
            Err(e) => e.to_string(),
        };
        assert!(e.contains(AUSENTE) && e.contains("cifra"), "{e}");
        // O cofre ligado RECUSA o arranque, como ja recusava -- agora dizendo
        // qual variavel. O erro sai antes de o cofre global ser tocado.
        let e = c.cifra.aplicar().unwrap_err().to_string();
        assert!(e.contains(AUSENTE), "o cofre recusou sem nomear: {e}");
        assert!(!e.contains("preencha"), "o erro velho, que mentia: {e}");
        assert!(
            c.avisos.iter().any(|a| a.contains(AUSENTE)),
            "a falta nao virou aviso: {:?}",
            c.avisos
        );
        assert_eq!(
            c.para_json()
                .campo("cifra")
                .map(|x| x.texto_ou("senha", "").to_string()),
            Some("(variavel ausente)".to_string())
        );
    }

    /// `cifra_fio.chave_privada_env` que falta NAO troca a identidade do
    /// servidor pela do arquivo.
    ///
    /// Era o pior dos cinco: a privada vazia fazia a busca seguir para o
    /// arquivo, e o servidor lia uma chave velha ou SORTEAVA e gravava uma
    /// nova -- o pino de todo cliente quebrava calado. A prova olha o disco:
    /// com o defeito reposto, `estatica` devolve `Ok` e o `chave-do-fio.hex`
    /// nasce.
    #[test]
    fn a_privada_do_fio_que_falta_no_ambiente_nao_vira_a_do_arquivo() {
        const AUSENTE: &str = "PHXSQL_TESTE_372_FIO_QUE_NINGUEM_EXPORTA";
        let d = DirTemp::novo("fio-372-ausente");
        let config = d.join("config.json");
        let j = Json::analisar(&format!(
            r#"{{"token":"t","cifra_fio":{{"chave_privada_env":"{AUSENTE}"}}}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).expect("a leitura NAO pode recusar pela falta");
        // `match`, e nao `expect_err`: com o defeito reposto o `Ok` traria
        // o valor, e o `expect_err` o imprimiria na saida do teste.
        let e = match c.cifra_fio.estatica(Some(&config)) {
            Ok(_) => panic!("a falta virou a chave do arquivo -- o defeito do 372"),
            Err(e) => e.to_string(),
        };
        assert!(e.contains(AUSENTE) && e.contains("cifra_fio"), "{e}");
        assert!(
            !d.join("chave-do-fio.hex").exists(),
            "a falta da variavel SORTEOU uma identidade nova no disco"
        );
        assert!(
            c.avisos.iter().any(|a| a.contains(AUSENTE)),
            "a falta nao virou aviso: {:?}",
            c.avisos
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// O comportamento VELHO da privada do fio: com a variavel PRESENTE, a
    /// chave e a dela, e nenhum arquivo nasce.
    #[test]
    fn a_privada_do_fio_com_a_variavel_presente_nada_muda() {
        const PRESENTE: &str = "PHXSQL_TESTE_372_FIO_PRESENTE";
        let privada = "99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa99aa";
        std::env::set_var(PRESENTE, privada);
        let d = DirTemp::novo("fio-372-presente");
        let config = d.join("config.json");
        let j = Json::analisar(&format!(
            r#"{{"token":"t","cifra_fio":{{"chave_privada_env":"{PRESENTE}"}}}}"#
        ))
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        let (k, avisos) = c.cifra_fio.estatica(Some(&config)).unwrap();
        assert!(avisos.is_empty(), "{avisos:?}");
        assert_eq!(phxsql_core::hash::para_hex(&k), privada);
        assert!(!d.join("chave-do-fio.hex").exists());
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
        let texto = c.para_json().escrever();
        assert!(!texto.contains(privada), "{texto}");
        assert!(texto.contains("(do ambiente)"), "{texto}");
        let _ = std::fs::remove_dir_all(&d);
    }

    // -----------------------------------------------------------------------
    // Pedido 372: a chave mestra do DbLink vem de FORA do conjunto copiado
    // -----------------------------------------------------------------------

    const CHAVE_372: &str = "a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0";

    /// (f) A chave mestra DENTRO da pasta que vai na copia do banco e recusada
    /// no arranque, com o motivo -- a do `config.json`, a dos dados e a do
    /// `dblink.json`, pelo caminho REAL (o `..` nao a esconde). Fora delas,
    /// sobe.
    ///
    /// O dano que a recusa impede e o que o vermelho mede: a copia da pasta
    /// levaria o cadastro cifrado E a chave que o abre -- a cifra protegeria
    /// contra ninguem, anunciando que protege.
    #[test]
    fn a_chave_mestra_dentro_da_pasta_do_banco_e_recusada_com_o_motivo() {
        let d = DirTemp::novo("config-372-chave-dentro");
        let fora = DirTemp::novo("config-372-chave-fora");
        let dados = DirTemp::novo("config-372-dados");
        let config = d.join("config.json");
        std::fs::write(d.join("chave.hex"), CHAVE_372).unwrap();
        std::fs::write(dados.join("chave.hex"), CHAVE_372).unwrap();
        std::fs::write(fora.join("chave.hex"), CHAVE_372).unwrap();
        let nome_de_d = d.file_name().unwrap().to_string_lossy().to_string();
        let casos = [
            // relativo: resolve ao lado do config.json
            ("chave.hex".to_string(), "a do config.json"),
            // a grafia com `..` que volta para dentro
            (
                fora.join("..")
                    .join(&nome_de_d)
                    .join("chave.hex")
                    .display()
                    .to_string(),
                "a do config.json",
            ),
            (dados.join("chave.hex").display().to_string(), "a dos dados"),
        ];
        for (caminho, qual) in casos {
            std::fs::write(
                &config,
                format!(
                    r#"{{"token":"t","base":{:?},
                         "cifra_do_dblink":{{"chave_mestra_arquivo":{caminho:?}}}}}"#,
                    dados.display().to_string()
                ),
            )
            .unwrap();
            let e = match Config::ler(&config) {
                Ok(c) => {
                    let na_copia = [d.join("chave.hex"), dados.join("chave.hex")]
                        .iter()
                        .filter(|p| p.exists())
                        .count();
                    panic!(
                        "a chave mestra em {caminho} foi ACEITA: a copia de {} leva o \
                         cadastro cifrado e {na_copia} arquivo(s) de chave que o abrem \
                         (campo {:?})",
                        d.display(),
                        c.cifra_do_dblink.campo
                    )
                }
                Err(e) => e.to_string(),
            };
            assert!(e.contains("DENTRO"), "{e}");
            assert!(
                e.contains(qual),
                "o motivo nao nomeia a pasta ({qual}): {e}"
            );
            assert!(!e.contains(CHAVE_372), "o erro imprimiu a chave: {e}");
        }

        // Fora das tres pastas, sobe -- e a chave fica disponivel.
        std::fs::write(
            &config,
            format!(
                r#"{{"token":"t","base":{:?},"cifra_do_dblink":{{"chave_mestra_arquivo":{:?}}}}}"#,
                dados.display().to_string(),
                fora.join("chave.hex").display().to_string()
            ),
        )
        .unwrap();
        let c = match Config::ler(&config) {
            Ok(c) => c,
            Err(e) => panic!("a chave FORA da pasta foi recusada: {e}"),
        };
        assert!(matches!(c.cifra_do_dblink.chave(), ChaveMestra::Pronta(_)));
        // O `Debug` do config nao carrega a chave.
        assert!(!format!("{c:?}").contains(CHAVE_372));
    }

    /// A chave escrita no PROPRIO `config.json`, e duas fontes ao mesmo tempo,
    /// sao recusadas na declaracao -- nomeando o campo.
    #[test]
    fn a_chave_mestra_no_config_json_ou_em_duas_fontes_e_recusada() {
        for (json, espera) in [
            (
                r#"{"token":"t","cifra_do_dblink":{"senha_mestra":"abc"}}"#,
                "cifra_do_dblink.senha_mestra",
            ),
            (
                r#"{"token":"t","cifra_do_dblink":{"chave_mestra":"00"}}"#,
                "cifra_do_dblink.chave_mestra",
            ),
            (
                r#"{"token":"t","cifra_do_dblink":{"senha_mestra_env":"A","chave_mestra_env":"B"}}"#,
                "UMA fonte",
            ),
            (
                r#"{"token":"t","cifra_do_dblink":{"senha_mestra_env":"A","iteracoes":5}}"#,
                "iteracoes",
            ),
        ] {
            let c = Config::de_json(&Json::analisar(json).unwrap()).unwrap();
            let e = c.validar().map(|_| ()).unwrap_err().to_string();
            assert!(e.contains(espera), "{json}: {e}");
            assert!(!e.contains("abc"), "o erro imprimiu o valor: {e}");
            // Nem o «campo desconhecido» junto: a recusa ja diz o motivo.
            assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
        }
        // O comportamento VELHO: sem a secao, nada muda -- nem aviso, nem
        // recusa, nem chave.
        let c = Config::de_json(&Json::analisar(r#"{"token":"t"}"#).unwrap()).unwrap();
        c.validar().unwrap();
        assert!(matches!(
            c.cifra_do_dblink.chave(),
            ChaveMestra::NaoDeclarada
        ));
    }

    /// (d, pelo `Config::ler`) A chave declarada e AUSENTE nao derruba nada: o
    /// `ler` sobe, e o aviso sai pela lista de sempre nomeando a variavel e as
    /// ligacoes trancadas -- nunca o valor.
    #[test]
    fn a_chave_mestra_ausente_avisa_pela_lista_de_sempre() {
        let d = DirTemp::novo("config-372-chave-ausente");
        let fora = DirTemp::novo("config-372-chave-ausente-fora");
        let config = d.join("config.json");
        // Cifra o cadastro com a chave num arquivo de fora...
        std::fs::write(fora.join("k.hex"), CHAVE_372).unwrap();
        std::fs::write(
            &config,
            format!(
                r#"{{"token":"t","cifra_do_dblink":{{"chave_mestra_arquivo":{:?}}}}}"#,
                fora.join("k.hex").display().to_string()
            ),
        )
        .unwrap();
        let c = Config::ler(&config).unwrap();
        let mut r = crate::dblink::Registro::abrir_com(&c.dblink, &c.cifra_do_dblink).unwrap();
        r.salvar(
            crate::dblink::Definicao::de_json(
                &Json::analisar(r#"{"nome":"loja","senha":"SEGREDO-372-AVISO"}"#).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        // ...e sobe com a variavel que ninguem exporta.
        const VAR: &str = "PHXSQL_TESTE_372_CONFIG_CHAVE_QUE_NINGUEM_EXPORTA";
        std::fs::write(
            &config,
            format!(r#"{{"token":"t","cifra_do_dblink":{{"chave_mestra_env":"{VAR}"}}}}"#),
        )
        .unwrap();
        let c = match Config::ler(&config) {
            Ok(c) => c,
            Err(e) => panic!("a chave AUSENTE derrubou a leitura do config: {e}"),
        };
        let aviso = c
            .avisos
            .iter()
            .find(|a| a.contains(VAR))
            .unwrap_or_else(|| panic!("a chave ausente subiu calada: {:?}", c.avisos));
        assert!(
            aviso.contains("TRANCADAS") && aviso.contains("\"loja\""),
            "{aviso}"
        );
        assert!(!aviso.contains("SEGREDO-372-AVISO"), "{aviso}");
    }

    /// (M1 da SEC) O link seguido de `..` NAO contorna a recusa: o caminho vai
    /// ao sistema operacional antes de o texto resolver o `..`.
    ///
    /// `fora/link -> <pasta do banco>/sub`, e a chave em
    /// `fora/link/../chave.hex`: pelo texto e `fora/chave.hex`, e o kernel abre
    /// `<pasta do banco>/chave.hex`. O teste prova as duas coisas contra o
    /// sistema, e com o defeito reposto (o `..` resolvido pelo texto primeiro)
    /// o vermelho diz que a chave de DENTRO foi aceita e lida.
    #[cfg(unix)]
    #[test]
    fn a_chave_por_link_seguido_de_ponto_ponto_e_recusada() {
        let banco = DirTemp::novo("config-372-m1-banco");
        let fora = DirTemp::novo("config-372-m1-fora");
        std::fs::create_dir_all(banco.join("sub")).unwrap();
        std::fs::write(banco.join("chave.hex"), CHAVE_372).unwrap();
        std::os::unix::fs::symlink(banco.join("sub"), fora.join("link")).unwrap();
        let pelo_link = format!("{}/link/../chave.hex", fora.display());
        // O que o kernel faz com esse caminho: abre a de DENTRO.
        assert_eq!(std::fs::read_to_string(&pelo_link).unwrap(), CHAVE_372);
        let config = banco.join("config.json");
        std::fs::write(
            &config,
            format!(
                r#"{{"token":"t","cifra_do_dblink":{{"chave_mestra_arquivo":{pelo_link:?}}}}}"#
            ),
        )
        .unwrap();
        let e = match Config::ler(&config) {
            Ok(c) => panic!(
                "a chave em {pelo_link} foi ACEITA: a conferencia viu {:?}, e o kernel \
                 abre esse caminho DENTRO da pasta do banco ({}) -- a chave viaja na \
                 copia junto com o cadastro (a leitura devolveu a chave: {})",
                c.cifra_do_dblink.conferido,
                banco.display(),
                matches!(c.cifra_do_dblink.chave(), ChaveMestra::Pronta(_))
            ),
            Err(e) => e.to_string(),
        };
        assert!(e.contains("DENTRO"), "{e}");
        assert!(!e.contains(CHAVE_372), "o erro imprimiu a chave: {e}");
    }

    /// (M1 da SEC) A chave se le do MESMO caminho que foi conferido: o
    /// diretorio trocado por um link para dentro da pasta do banco DEPOIS da
    /// conferencia nao leva a leitura para la.
    ///
    /// Com o defeito reposto (ler sem refazer o caminho real), a chave de
    /// dentro volta como se fosse a de fora -- e o vermelho diz isso.
    #[cfg(unix)]
    #[test]
    fn a_chave_nao_se_le_de_um_caminho_que_mudou_depois_da_conferencia() {
        const DE_DENTRO: &str = "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1";
        let banco = DirTemp::novo("config-372-m1b-banco");
        let fora = DirTemp::novo("config-372-m1b-fora");
        std::fs::create_dir_all(fora.join("sub")).unwrap();
        std::fs::write(fora.join("sub").join("k.hex"), CHAVE_372).unwrap();
        std::fs::write(banco.join("k.hex"), DE_DENTRO).unwrap();
        let config = banco.join("config.json");
        std::fs::write(
            &config,
            format!(
                r#"{{"token":"t","cifra_do_dblink":{{"chave_mestra_arquivo":{:?}}}}}"#,
                fora.join("sub").join("k.hex").display().to_string()
            ),
        )
        .unwrap();
        let c = match Config::ler(&config) {
            Ok(c) => c,
            Err(e) => panic!("a chave FORA da pasta foi recusada: {e}"),
        };
        assert!(matches!(c.cifra_do_dblink.chave(), ChaveMestra::Pronta(_)));
        // Depois da conferencia, o diretorio vira um link para dentro do banco.
        std::fs::rename(fora.join("sub"), fora.join("sub-velho")).unwrap();
        std::os::unix::fs::symlink(&banco.0, fora.join("sub")).unwrap();
        match c.cifra_do_dblink.chave() {
            ChaveMestra::Pronta(k) if k == [0xb1; 32] => panic!(
                "a chave foi lida de DENTRO da pasta do banco ({}), pelo caminho que \
                 tinha passado na conferencia",
                banco.display()
            ),
            ChaveMestra::Indisponivel(m) => assert!(m.contains("mudou"), "{m}"),
            outra => panic!("esperava a recusa do caminho que mudou: {outra:?}"),
        }
    }

    /// (B3 da SEC) `cifra_do_dblink` escrita torta RECUSA, como toda declaracao
    /// da casa -- e nunca vira «nao declarada», que deixaria o cadastro em
    /// claro com o dono achando que cifrou.
    ///
    /// Com o defeito reposto (sem a conferencia de tipo), o vermelho diz que a
    /// declaracao sumiu calada.
    #[test]
    fn cifra_do_dblink_escrita_torta_recusa_e_nao_vira_nao_declarada() {
        for (json, espera) in [
            (r#"{"token":"t","cifra_do_dblink":"s3nh4"}"#, "objeto"),
            (
                r#"{"token":"t","cifra_do_dblink":{"chave_mestra_arquivo":123}}"#,
                "texto",
            ),
            (
                r#"{"token":"t","cifra_do_dblink":{"senha_mestra_env":["X"]}}"#,
                "texto",
            ),
            (
                r#"{"token":"t","cifra_do_dblink":{"senha_mestra_env":"X","iteracoes":"muitas"}}"#,
                "inteiro",
            ),
        ] {
            let c = Config::de_json(&Json::analisar(json).unwrap()).unwrap();
            if matches!(c.cifra_do_dblink.chave(), ChaveMestra::NaoDeclarada) {
                panic!(
                    "{json}: a declaracao torta virou «nao declarada» -- quem a \
                     escreveu acha que cifrou, e o cadastro continua em claro sem \
                     recusa nenhuma"
                );
            }
            let e = match c.validar() {
                Ok(()) => panic!("{json}: a declaracao torta subiu"),
                Err(e) => e.to_string(),
            };
            assert!(e.contains(espera), "{json}: {e}");
            assert!(!e.contains("s3nh4"), "o erro repetiu o valor torto: {e}");
        }
    }

    /// O aviso do DbLink chega pela lista de sempre -- a do `Config::ler`, que
    /// o `main` imprime -- e nao por um segundo mecanismo.
    #[test]
    fn o_ler_traz_o_aviso_do_texto_puro_do_dblink() {
        let d = DirTemp::novo("config-372-dblink");
        let config = d.join("config.json");
        std::fs::write(&config, r#"{"token":"t"}"#).unwrap();
        // Sem cadastro: nada, que e o arranque de quase todo servidor.
        let c = Config::ler(&config).unwrap();
        assert!(
            !c.avisos.iter().any(|a| a.contains("DbLink")),
            "{:?}",
            c.avisos
        );

        std::fs::write(
            d.join("dblink.json"),
            r#"{"dblink":[{"nome":"loja","senha":"SEGREDO-372"}]}"#,
        )
        .unwrap();
        let c = Config::ler(&config).unwrap();
        let aviso = c
            .avisos
            .iter()
            .find(|a| a.contains("TEXTO PURO"))
            .unwrap_or_else(|| panic!("o texto puro subiu calado: {:?}", c.avisos));
        assert!(aviso.contains("\"loja\" (senha)"), "{aviso}");
        assert!(
            !aviso.contains("SEGREDO-372"),
            "o aviso imprimiu a senha: {aviso}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_senha_da_cifra_pode_vir_do_ambiente() {
        std::env::set_var("PHXSQL_TESTE_CIFRA", "vinda do ambiente");
        let j = Json::analisar(
            r#"{"token":"t","cifra":{"ligada":true,"senha_env":"PHXSQL_TESTE_CIFRA"}}"#,
        )
        .unwrap();
        let c = Config::de_json(&j).unwrap();
        assert_eq!(c.cifra.senha().unwrap(), "vinda do ambiente");
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

    /// Pedido 225. Os SEIS caminhos padrao (`dados`, `acessos.log`,
    /// `blacklist.json`, `dblink.json`, `jobs.json`, `backups`) moram ao lado
    /// do `config.json` -- nunca do diretorio de trabalho do processo.
    ///
    /// `dblink` e `jobs` sao os dois que este teste existe para travar: ate
    /// este pedido, so `base`, `log_acessos`, `blacklist` e `backup.destino`
    /// passavam por `Config::ler`'s bloco de resolucao -- os outros dois
    /// entraram no `Config` DEPOIS daquele bloco existir, e um `phxsqld`
    /// iniciado de outro diretorio espalhava `jobs.json`/`jobs.log` (e
    /// `dblink.json`) onde estivesse, nao ao lado do config. Foi assim que a
    /// bancada `bancada/proibidos/provar.py` deixou `jobs.json` na raiz do
    /// repositorio.
    #[test]
    fn caminhos_padrao_resolvem_contra_o_diretorio_do_config() {
        let dir = DirTemp::novo("caminhos-padrao");
        let caminho = dir.join("config.json");
        std::fs::write(&caminho, r#"{"token":"t"}"#).unwrap();

        let c = Config::ler(&caminho).unwrap();
        assert_eq!(c.base, dir.join("dados"));
        assert_eq!(c.log_acessos, dir.join("acessos.log"));
        assert_eq!(c.blacklist, dir.join("blacklist.json"));
        assert_eq!(c.dblink, dir.join("dblink.json"), "dblink ficou de fora");
        assert_eq!(c.jobs, dir.join("jobs.json"), "jobs ficou de fora");
        assert_eq!(c.backup.destino, dir.join("backups"));
    }

    /// O comportamento VELHO nao pode mudar: caminho ABSOLUTO no
    /// `config.json` continua absoluto, e nao ganha o diretorio do config na
    /// frente. Sem este teste, `caminhos_padrao_resolvem_contra_o_diretorio_
    /// do_config` sozinho passaria com uma guarda burra que sempre concatena.
    #[test]
    fn caminho_absoluto_no_config_continua_absoluto() {
        let dir = DirTemp::novo("caminho-absoluto");
        let caminho = dir.join("config.json");
        let alhures = DirTemp::novo("caminho-absoluto-alhures");
        let texto = format!(
            r#"{{"token":"t","jobs":{jobs:?},"dblink":{dblink:?}}}"#,
            jobs = alhures.join("j.json").display().to_string(),
            dblink = alhures.join("d.json").display().to_string(),
        );
        std::fs::write(&caminho, texto).unwrap();

        let c = Config::ler(&caminho).unwrap();
        assert_eq!(c.jobs, alhures.join("j.json"));
        assert_eq!(c.dblink, alhures.join("d.json"));
    }

    /// Prova real do defeito, no sentido que importa: com `jobs` e `dblink`
    /// fora do bloco de resolucao (o estado antes deste pedido), a funcao
    /// devolve o caminho INTOCADO -- relativo, do jeito que so faz sentido
    /// contra o cwd do processo. Reproduz o defeito sem precisar reverter o
    /// codigo: chama `resolver_caminho_do_config` a mesma funcao que
    /// `Config::ler` usa hoje, e o outro teste (`caminhos_padrao_resolvem_
    /// contra_o_diretorio_do_config`) e quem prova que ela ESTA sendo chamada
    /// para os seis campos.
    #[test]
    fn resolver_caminho_do_config_ancora_no_diretorio_do_config_em() {
        let dir = DirTemp::novo("resolver");
        let config_em = dir.join("config.json");
        assert_eq!(
            resolver_caminho_do_config(&PathBuf::from("jobs.json"), Some(&config_em)),
            dir.join("jobs.json"),
        );
        // Absoluto nao muda.
        let abs = PathBuf::from("/etc/em/algum/lugar.json");
        assert_eq!(resolver_caminho_do_config(&abs, Some(&config_em)), abs);
        // Sem config_em, fica como esta -- e o cwd do processo quem decide.
        assert_eq!(
            resolver_caminho_do_config(&PathBuf::from("jobs.json"), None),
            PathBuf::from("jobs.json"),
        );
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
        // Uma origem com lista e outra SEM, e o `validar()` aprova -- e
        // continua aprovando depois da guarda de sobreposicao (pedido 406).
        // Lista vazia quer dizer «todos os databases daquela origem», e o que
        // saopaulo tem so saopaulo sabe: recusar aqui seria palpite, e o
        // palpite tiraria do ar o proprio `Config_exemplo_03.json`. A recusa
        // deste caso acontece na DESCOBERTA, por database e sem derrubar os
        // outros -- `Servidor::so_os_databases_desta_origem`.
        c.validar().unwrap();
    }

    /// Duas origens declarando o MESMO database nao sobem, e a recusa nomeia
    /// as DUAS origens e o database -- pedido 406, parecer do papel C de
    /// 23/09/2026 secao 6. Antes desta guarda as duas subiam e escreviam no
    /// mesmo `.reg` daqui: a inclusao fail-stopava no meio do expediente e a
    /// alteracao sobrescrevia calada.
    #[test]
    fn duas_origens_com_o_mesmo_database_nao_sobem() {
        let txt = r#"{
          "token":"x",
          "replicacao":{
            "papel":"replica",
            "origens":[
              {"nome":"caixa01","host":"10.1.1.1","porta":5000,"token":"t1",
               "databases":["vendas","precos"]},
              {"nome":"caixa02","host":"10.1.1.2","porta":5000,"token":"t2",
               "databases":["estoque","vendas"]}
            ]
          }
        }"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let e = c.validar().unwrap_err().to_string();
        assert!(
            e.contains("caixa01"),
            "a recusa nao nomeou a 1a origem: {e}"
        );
        assert!(
            e.contains("caixa02"),
            "a recusa nao nomeou a 2a origem: {e}"
        );
        assert!(e.contains("vendas"), "a recusa nao nomeou o database: {e}");
        // E nomeia o REPETIDO, nao um qualquer das listas.
        assert!(
            !e.contains("precos") && !e.contains("estoque"),
            "a recusa nomeou database que nao se repete: {e}"
        );
    }

    /// A medida de «quantos arquivos do produto a guarda quebraria», feita
    /// pelo `validar()` e nao por leitura: os QUATRO `config.json` que o
    /// repositorio entrega sobem. Guarda nova entra pedida, nao imposta, e o
    /// jeito de saber se ela foi imposta e este -- o exemplo 03 e o do
    /// docker trazem origem de lista vazia, que e justamente o que as
    /// recusas novas poderiam ter matado por palpite.
    ///
    /// O do docker entra por `include_str!` como os outros tres: lista de
    /// arquivos digitada no teste envelhece calada quando alguem acrescenta
    /// um exemplo, e o compilador nao reclama de arquivo que ninguem citou.
    #[test]
    fn os_config_de_exemplo_do_repositorio_continuam_subindo() {
        const DOCKER: &str = include_str!("../../../exemplos/Config_docker_replica.json");
        for (nome, texto) in [
            ("Config_exemplo_01.json", crate::CONFIG_EXEMPLO_01),
            ("Config_exemplo_02.json", crate::CONFIG_EXEMPLO_02),
            ("Config_exemplo_03.json", crate::CONFIG_EXEMPLO_03),
            ("Config_docker_replica.json", DOCKER),
        ] {
            let c = Config::de_json(&Json::analisar(texto).unwrap()).unwrap();
            c.validar()
                .unwrap_or_else(|e| panic!("{nome} deixou de subir: {e}"));
        }
    }

    /// **ALTO-1 da revisao adversaria do 406.** O campo `"nome"` e opcional e
    /// o padrao e `"origem"`: duas origens que so OMITEM o campo viram as duas
    /// `"origem"`, e ai a guarda dinamica compara `dono.origem == origem`,
    /// acha verdadeiro para as duas e as deixa passar -- a guarda inteira
    /// desligada por um campo que ninguem preencheu. Nao precisa de malicia:
    /// basta omitir um campo opcional duas vezes.
    ///
    /// **Prova real:** tire a chamada a `recusar_nome_de_origem_repetido` do
    /// `recusar_origens_ambiguas` e este teste reprova no `unwrap_err`.
    #[test]
    fn duas_origens_sem_nome_nao_sobem() {
        let txt = r#"{"token":"x","somente_leitura":true,
          "replicacao":{"papel":"replica","origens":[
            {"host":"10.1.1.1","porta":5000,"token":"t1","databases":["vendas"]},
            {"host":"10.1.1.2","porta":5000,"token":"t2","databases":["estoque"]}]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        // O padrao que cria o buraco, conferido aqui para que ninguem precise
        // acreditar: as duas nascem com o MESMO nome.
        assert_eq!(c.replicacao.origens[0].nome, "origem");
        assert_eq!(c.replicacao.origens[1].nome, "origem");
        let e = c.validar().unwrap_err().to_string();
        // Os databases sao DISJUNTOS: quem recusa aqui e o nome, nao a
        // sobreposicao -- se fosse a sobreposicao o teste provaria outra coisa.
        assert!(
            e.contains("10.1.1.1:5000") && e.contains("10.1.1.2:5000"),
            "a recusa tem de nomear as duas origens, e o nome nao as distingue: {e}"
        );
        assert!(
            e.contains("\"nome\""),
            "a recusa nao disse qual campo falta: {e}"
        );
    }

    /// **ALTO-3 da revisao adversaria do 406.** Duas origens de lista vazia
    /// sao indecidiveis: lista vazia quer dizer «todos os databases desta
    /// origem», entao o dono de um nome que as duas entreguem seria a thread
    /// que conectasse primeiro -- outro a cada arranque, por latencia de rede.
    /// Antes da guarda as duas escreviam juntas e o fail-stop chegava cedo;
    /// sortear o vencedor teria trocado uma falha cedo por uma falha
    /// intermitente, que e pior de diagnosticar. Entao o arranjo se recusa.
    ///
    /// **Prova real:** tire a chamada a `recusar_duas_origens_sem_databases`
    /// e este teste reprova no `unwrap_err`.
    #[test]
    fn duas_origens_sem_databases_nao_sobem() {
        let txt = r#"{"token":"x","somente_leitura":true,
          "replicacao":{"papel":"replica","origens":[
            {"nome":"alfa","host":"10.1.1.1","porta":5000,"token":"t1"},
            {"nome":"beta","host":"10.1.1.2","porta":5000,"token":"t2"}]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let e = c.validar().unwrap_err().to_string();
        assert!(
            e.contains("alfa") && e.contains("beta"),
            "a recusa nao nomeou as duas origens: {e}"
        );
        assert!(
            e.contains("databases"),
            "a recusa nao disse o que resolve -- declarar as listas: {e}"
        );
    }

    /// **ALTO-2 da revisao adversaria do 406, no nivel do `Config`.** Com o
    /// bloco `cluster` a lista `replicacao.origens` e IGNORADA (o servidor
    /// avisa e puxa do master corrente), entao NENHUMA das tres recusas pode
    /// ligar: um no que tinha origens sobrando de antes deixaria de subir por
    /// causa de uma lista que ninguem le. O irmao deste teste, do lado do
    /// servidor, e `com_cluster_a_guarda_nao_liga`.
    ///
    /// **Prova real:** tire o portao (`if !self.puxa_de_varias_origens()`) do
    /// `recusar_origens_ambiguas` e este teste reprova nos tres arranjos.
    #[test]
    fn com_cluster_nenhuma_das_tres_recusas_liga() {
        let cluster = r#","cluster":{"id":"no1","janela_inatividade_s":30,
              "nos":[{"id":"no1","endereco":"127.0.0.1","porta":5399},
                     {"id":"no2","endereco":"127.0.0.1","porta":5398}]}"#;
        for (caso, origens) in [
            (
                "mesmo database declarado",
                r#"{"nome":"a","host":"10.1.1.1","porta":5000,"token":"t","databases":["loja"]},
                   {"nome":"b","host":"10.1.1.2","porta":5000,"token":"t","databases":["loja"]}"#,
            ),
            (
                "nomes homonimos",
                r#"{"host":"10.1.1.1","porta":5000,"token":"t","databases":["loja"]},
                   {"host":"10.1.1.2","porta":5000,"token":"t","databases":["caixa"]}"#,
            ),
            (
                "duas listas vazias",
                r#"{"nome":"a","host":"10.1.1.1","porta":5000,"token":"t"},
                   {"nome":"b","host":"10.1.1.2","porta":5000,"token":"t"}"#,
            ),
        ] {
            let txt = format!(
                r#"{{"token":"x","somente_leitura":true,
                     "replicacao":{{"papel":"replica","id_servidor":"no1",
                       "origens":[{origens}]}}{cluster}}}"#
            );
            let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
            c.validar()
                .unwrap_or_else(|e| panic!("o no de cluster deixou de subir ({caso}): {e}"));
        }
    }

    /// E o mesmo portao pelo outro lado: sem cluster, um papel que NAO puxa
    /// (source, isolado) tambem nao sobe laco nenhum, e a lista de origens
    /// sobrando nao pode derrubar o servidor.
    #[test]
    fn papel_que_nao_puxa_nao_e_tocado_pelas_recusas() {
        let txt = r#"{"token":"x",
          "replicacao":{"papel":"source","origens":[
            {"nome":"a","host":"10.1.1.1","porta":5000,"token":"t","databases":["loja"]},
            {"nome":"a","host":"10.1.1.2","porta":5000,"token":"t","databases":["loja"]}]}}"#;
        Config::de_json(&Json::analisar(txt).unwrap())
            .unwrap()
            .validar()
            .unwrap();
    }

    /// O teste do comportamento VELHO, que e o que mais importa numa guarda
    /// nova: vinte caixas com vinte nomes de database -- o arranjo ESPELHO,
    /// que e justamente o que esta guarda existe para tornar seguro -- sobem
    /// como sempre subiram. Guarda nova entra pedida, nao imposta.
    #[test]
    fn vinte_origens_com_nomes_distintos_continuam_subindo() {
        let origens: Vec<String> = (1..=20)
            .map(|i| {
                format!(
                    r#"{{"nome":"caixa{i:02}","host":"10.1.1.{i}","porta":5000,
                        "token":"t{i}","databases":["caixa{i:02}"]}}"#
                )
            })
            .collect();
        let txt = format!(
            r#"{{"token":"x","somente_leitura":true,
                 "replicacao":{{"papel":"replica","origens":[{}]}}}}"#,
            origens.join(",")
        );
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        assert_eq!(c.replicacao.origens.len(), 20);
        c.validar().unwrap();
    }

    /// O par 1<->1 nao tem com quem cruzar, e e a configuracao mais comum que
    /// existe: ela tem de subir byte a byte.
    #[test]
    fn o_par_1_para_1_nao_e_tocado_pela_guarda() {
        let txt = r#"{"token":"x","somente_leitura":true,
          "replicacao":{"papel":"replica","origens":[
            {"nome":"master","host":"10.0.0.7","porta":5000,"token":"t"}]}}"#;
        Config::de_json(&Json::analisar(txt).unwrap())
            .unwrap()
            .validar()
            .unwrap();
    }

    /// O exemplo que acompanha o produto tem TRES origens, duas com lista e
    /// uma sem (curitiba `["Z"]`, saopaulo vazia, bruxelas `["W"]`). Ele e o
    /// arranjo legitimo que uma recusa por palpite mataria -- e por isso ele
    /// esta aqui, e nao so no teste de campo estranho.
    #[test]
    fn o_exemplo_da_replica_continua_valido() {
        let j = Json::analisar(crate::CONFIG_EXEMPLO_03).unwrap();
        let c = Config::de_json(&j).unwrap();
        assert!(c.replicacao.origens.len() >= 3, "o exemplo mudou de forma");
        assert!(
            c.replicacao.origens.iter().any(|o| o.databases.is_empty()),
            "o exemplo deixou de ter origem com lista vazia: a guarda do 406 \
             perdeu a prova de que nao recusa por palpite"
        );
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

    // ================================================= o webservice REST

    /// **O teste que mais importa desta frente inteira**: o comportamento
    /// VELHO.
    ///
    /// Um `config.json` de hoje -- e todos sao de hoje -- nao tem a secao
    /// `rest`. Depois de atualizar o binario ele nao pode passar a escutar
    /// porta nenhuma: isso seria abrir superficie de ataque sem ninguem pedir,
    /// e a regra da casa e petrea -- guarda nova, e porta nova, entra PEDIDA.
    ///
    /// **Prova real, com o defeito reposto:** troque o `booleano_ou("ligado",
    /// false)` do `Rest::de_json` por `true` -- ou faca o `Default` nascer
    /// ligado -- e este teste reprova nos dois campos.
    #[test]
    fn config_sem_a_secao_rest_nao_escuta() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!c.rest.ligado, "o REST subiu sem ninguem pedir");
        assert!(
            !c.rest.swagger_ligado,
            "o explorador subiu sem ninguem pedir"
        );
        // E os enderecos de fabrica sao do proprio computador: no dia em que
        // alguem ligar, ligar nao pode significar publicar na rede.
        assert!(c.rest.bind.starts_with("127.0.0.1:"));
        assert!(c.rest.swagger_bind.starts_with("127.0.0.1:"));
        // Nada estreitado, nada exigido, nenhum segredo proprio.
        assert!(c.rest.database.is_empty());
        assert!(c.rest.tabelas.is_empty());
        assert!(c.rest.token.is_empty());
        // E a secao ausente nao vira aviso de campo desconhecido.
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    /// As duas portas sao independentes: o REST sem o explorador e o caso da
    /// placa, e ele tem de ser dizivel numa linha do arquivo.
    #[test]
    fn o_rest_liga_sem_o_explorador() {
        let c = Config::de_json(
            &Json::analisar(
                r#"{"token":"x","rest":{"ligado":true,"bind":"127.0.0.1:7500",
                    "nome":"vendas","database":"loja","tabelas":["clientes"," "],
                    "token":"outro"}}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(c.rest.ligado);
        assert!(!c.rest.swagger_ligado, "o explorador nao foi pedido");
        assert_eq!(c.rest.endereco().unwrap().port(), 7500);
        assert_eq!(c.rest.titulo(), "vendas");
        // Item em branco na lista nao vira uma tabela chamada "".
        assert_eq!(c.rest.tabelas, vec!["clientes".to_string()]);
    }

    /// Duas portas no mesmo endereco nao sobem, e descobrir isso no arranque e
    /// melhor do que descobrir com uma delas calada.
    #[test]
    fn as_portas_do_rest_entram_na_conta_das_colisoes() {
        let erro = Config::de_json(
            &Json::analisar(
                r#"{"token":"x","bind":"127.0.0.1:7501",
                    "rest":{"ligado":true,"bind":"127.0.0.1:7501"}}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .validar()
        .unwrap_err()
        .to_string();
        assert!(erro.contains("rest.bind"), "{erro}");

        let erro = Config::de_json(
            &Json::analisar(
                r#"{"token":"x","rest":{"ligado":true,"bind":"127.0.0.1:7502",
                    "swagger_ligado":true,"swagger_bind":"127.0.0.1:7502"}}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .validar()
        .unwrap_err()
        .to_string();
        assert!(erro.contains("swagger_bind"), "{erro}");

        // E a porta DESLIGADA nao colide com nada: quem deixou o campo
        // apontando para a porta de dados e nao ligou o REST nao pode ficar
        // sem servidor por causa de um endereco que ninguem vai abrir.
        Config::de_json(
            &Json::analisar(
                r#"{"token":"x","bind":"127.0.0.1:7503",
                    "rest":{"bind":"127.0.0.1:7503"}}"#,
            )
            .unwrap(),
        )
        .unwrap()
        .validar()
        .expect("porta desligada nao colide");
    }

    /// O token do REST nao sai na resposta de `config`, e nao entra pela tela.
    ///
    /// Senha nunca em texto puro vale para segredo de porta tambem: manda-lo
    /// para a tela o poria no historico do navegador, no log do proxy e na
    /// captura de tela do suporte. E a tela nao o grava pelo mesmo motivo que
    /// ja mantinha `token` fora: uma sessao tomada nao troca a fechadura.
    #[test]
    fn o_token_do_rest_nao_sai_nem_entra_pela_tela() {
        let c = Config::de_json(
            &Json::analisar(r#"{"token":"x","rest":{"token":"SEGREDO-DA-PORTA"}}"#).unwrap(),
        )
        .unwrap();
        let visto = c.para_json().escrever();
        assert!(
            !visto.contains("SEGREDO-DA-PORTA"),
            "o token vazou para a tela"
        );
        let rest = c.para_json().campo("rest").cloned().expect("secao rest");
        assert!(
            rest.booleano_ou("token_proprio", false),
            "a tela precisa saber que existe um"
        );
        assert!(rest.campo("token").is_none());
        assert!(
            campo_editavel("rest.token").is_none(),
            "a tela nao grava credencial"
        );
        // Os que a tela GRAVA continuam gravaveis, senao a secao inteira
        // viraria somente leitura por engano.
        for campo in [
            "rest.ligado",
            "rest.bind",
            "rest.nome",
            "rest.database",
            "rest.tabelas",
            "rest.swagger_ligado",
            "rest.swagger_bind",
        ] {
            assert!(campo_editavel(campo).is_some(), "{campo} sumiu da tela");
        }
    }

    /// O tipo `lista` recusa o que nao e lista de textos.
    ///
    /// Sem a conferencia, gravar `"rest.tabelas": "clientes"` passaria: o
    /// leitor usa `textos()`, que devolve vazio calado quando o tipo nao bate
    /// -- e vazio, aqui, quer dizer "expoe tudo". Um erro de digitacao na tela
    /// abriria as trinta tabelas de quem quis expor tres.
    #[test]
    fn a_lista_de_tabelas_so_aceita_lista_de_textos() {
        let (tipo, _) = campo_editavel("rest.tabelas").unwrap();
        assert!(tipo.confere(&Json::analisar(r#"["a","b"]"#).unwrap()));
        assert!(tipo.confere(&Json::analisar("[]").unwrap()));
        assert!(!tipo.confere(&Json::texto_de("clientes")));
        assert!(!tipo.confere(&Json::analisar("[1,2]").unwrap()));
        assert!(!tipo.confere(&Json::analisar(r#"{"a":1}"#).unwrap()));
    }

    /// Campo escrito errado dentro da secao vira aviso, e nao silencio.
    #[test]
    fn campo_estranho_dentro_do_rest_avisa() {
        let c =
            Config::de_json(&Json::analisar(r#"{"token":"x","rest":{"ligada":true}}"#).unwrap())
                .unwrap();
        assert!(
            c.estranhas.iter().any(|e| e == "rest.ligada"),
            "{:?}",
            c.estranhas
        );
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

    /// Pedido 228: sem as secoes `acessos`/`diretivas`, o rodizio nasce
    /// DESLIGADO -- ao contrario do Profiler, que nasce ligado por decisao
    /// propria. E o que "guarda nova entra pedida" exige aqui: um
    /// `config.json` de antes do pedido 228 nao pode passar a girar sozinho.
    #[test]
    fn sem_as_secoes_acessos_e_diretivas_nascem_sem_rodizio() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert_eq!(c.acessos.arquivo_mib, 0);
        assert_eq!(c.acessos.arquivos, 0);
        assert_eq!(c.acessos.teto_do_arquivo(), 0);
        assert_eq!(c.diretivas.arquivo_mib, 0);
        assert_eq!(c.diretivas.arquivos, 0);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    /// Configurados, os dois lem exatamente como o Profiler -- mesmo tipo,
    /// mesma unidade (MiB vira bytes), e o `teto_em_disco_mib` sai calculado
    /// e nao precisa ser conferido no JavaScript da tela.
    #[test]
    fn acessos_e_diretivas_configurados_leem_como_o_profiler() {
        let txt = r#"{"token":"x",
            "acessos":{"arquivo_mib":32,"arquivos":3},
            "diretivas":{"arquivo_mib":8,"arquivos":2}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.acessos.arquivo_mib, 32);
        assert_eq!(c.acessos.arquivos, 3);
        assert_eq!(c.acessos.teto_do_arquivo(), 32 * 1024 * 1024);
        assert_eq!(c.diretivas.arquivo_mib, 8);
        assert_eq!(c.diretivas.arquivos, 2);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);

        let j = c.para_json();
        let a = j.campo("acessos").unwrap();
        assert_eq!(a.inteiro_ou("arquivo_mib", -1), 32);
        assert_eq!(a.inteiro_ou("teto_em_disco_mib", -1), 32 * (3 + 1));
        let d = j.campo("diretivas").unwrap();
        assert_eq!(d.inteiro_ou("arquivo_mib", -1), 8);
        assert_eq!(d.inteiro_ou("teto_em_disco_mib", -1), 8 * (2 + 1));
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

    /// O irmao do REST responde igual ao portao 1 -- e pelo mesmo laco.
    /// (Tempo constante nao se prova por teste de unidade; prova-se por
    /// construcao, e a construcao e uma so, no `hash.rs`.)
    #[test]
    fn o_bearer_do_rest_confere_como_o_token_do_protocolo() {
        let r = Rest {
            token: "abc123".into(),
            ..Rest::default()
        };
        assert!(r.token_confere("abc123"));
        assert!(!r.token_confere("abc124"));
        assert!(!r.token_confere("abc"));
        assert!(!r.token_confere("abc1234"));
        assert!(!r.token_confere(""));
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

    /// **O PADRAO no arquivo, e ele VIROU em 18/09/2026.**
    ///
    /// Este teste se chamava `cifra_do_cluster_nasce_desligada` e travava o
    /// contrario: um cluster sem `cifra` nascia em claro. A ordem do dono
    /// ("a comunicacao deve obrigatoriamente ser cifrada") alcanca a saida
    /// tambem, e o nome mudou junto com o significado -- teste que muda de
    /// veredito e fica com o nome antigo e um teste que mente para quem le a
    /// lista.
    ///
    /// O que NAO mudou, e por isso continua travado aqui: o PINO nao nasce.
    /// Cifra de fabrica protege da escuta passiva; pino e afirmacao sobre a
    /// identidade do outro lado, e afirmacao dessas nao se herda de um padrao.
    #[test]
    fn cifra_do_cluster_nasce_ligada() {
        let txt = cluster_minimo("");
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let cl = c.cluster.unwrap();
        assert!(cl.cifra, "o cluster de fabrica ficou falando em claro");
        assert!(
            cl.nos.iter().all(|n| n.chave_do_fio.is_empty()),
            "no do cluster nasceu com pino sem ninguem pedir"
        );
    }

    /// **O COMPORTAMENTO VELHO, pelo escape ESCRITO.** Um cluster com um no de
    /// versao anterior ao aperto escreve `"cifra": false` e continua em claro,
    /// exatamente como antes da virada -- e e a unica maneira de isso
    /// acontecer, que e o ponto do escape.
    #[test]
    fn o_escape_escrito_deixa_o_cluster_em_claro() {
        let txt = cluster_minimo(r#""cifra":false,"#);
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        assert!(
            !c.cluster.unwrap().cifra,
            "o escape escrito nao foi obedecido"
        );
    }

    /// Um cluster de tres nos com a cifra e os pinos escritos a mao -- nao
    /// reusa o `cluster_minimo` porque a lista de nos muda, e a `chave_do_fio`
    /// mora dentro de cada no.
    fn cluster_cifrado(cifra: bool, pino2: &str) -> String {
        format!(
            r#"{{"token":"t","replicacao":{{"papel":"source"}},
                "cluster":{{"id":"no1","janela_inatividade_s":6,"cifra":{cifra},
                  "nos":[
                    {{"id":"no1","endereco":"127.0.0.1","porta":5310}},
                    {{"id":"no2","endereco":"127.0.0.1","porta":5311,"chave_do_fio":"{pino2}"}},
                    {{"id":"no3","endereco":"127.0.0.1","porta":5312,"chave_do_fio":"{}"}}]}}}}"#,
            "bb".repeat(32),
        )
    }

    /// Ligada e com pino por no: os dois campos chegam ao `Cluster`, e o pino
    /// vira os 32 bytes da X25519 sem passar pela leitura de nenhum laco.
    #[test]
    fn cifra_do_cluster_ligada_le_o_pino_por_no() {
        let pino2 = "aa".repeat(32);
        let txt = cluster_cifrado(true, &pino2);
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let cl = c.cluster.as_ref().unwrap();
        assert!(cl.cifra);
        assert_eq!(cl.no("no1").unwrap().pino_do_fio().unwrap(), None);
        assert_eq!(
            cl.no("no2").unwrap().pino_do_fio().unwrap(),
            Some([0xaau8; 32])
        );
        assert_eq!(
            cl.no("no3").unwrap().pino_do_fio().unwrap(),
            Some([0xbbu8; 32])
        );
        c.validar().unwrap();
        // O pino NUNCA sai pela op `config` -- so o fato de haver um.
        let resp = c.para_json().escrever();
        assert!(!resp.contains(&pino2), "o pino vazou na resposta: {resp}");
        assert!(resp.contains("\"tem_pino\":true"), "{resp}");
        assert!(resp.contains("\"cifra\":true"), "{resp}");
    }

    /// Pino torto e recusado na DECLARACAO, com o no nomeado -- e nao num
    /// pulso qualquer daqui a tres semanas. E a irma do
    /// `pino_torto_na_origem_e_erro_e_nao_ausencia`, do lado do cluster: vale
    /// ate com a cifra desligada, porque um pino guardado para ligar depois
    /// nao pode estar errado esperando o dia.
    #[test]
    fn pino_torto_no_no_do_cluster_e_erro_e_nao_ausencia() {
        // Cifra DESLIGADA de proposito: o pino torto ainda derruba a validacao.
        let txt = cluster_cifrado(false, "abacaxi");
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let erro = c.validar().unwrap_err();
        let msg = erro.to_string();
        assert!(msg.contains("no2"), "o erro nao nomeou o no torto: {msg}");
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

    /// **O padrao da porta web e o LACO LOCAL, e isso e uma guarda.**
    ///
    /// Ordem do dono, 18/09/2026: a comunicacao tem de ser cifrada, e para o
    /// navegador a saida e o proxy reverso na frente (`docs/SEGURANCA.md`
    /// §7.1). Proxy so protege se o motor NAO estiver aberto ao lado dele --
    /// senao o atacante liga direto e pula o TLS inteiro.
    ///
    /// O teste confere as DUAS formas de nao declarar endereco, porque sao
    /// caminhos diferentes no leitor: arquivo sem a secao `web` (cai no
    /// `Web::default`) e arquivo COM a secao e sem o campo `bind` (cai no
    /// `texto_ou`, que e outra linha). A segunda e a que envelheceria calada.
    #[test]
    fn a_porta_web_sem_endereco_declarado_nasce_no_laco_local() {
        for txt in [
            r#"{"token":"x"}"#,
            r#"{"token":"x","web":{"ligado":true}}"#,
            r#"{"token":"x","web":{"ligado":true,"sessao_minutos":15}}"#,
        ] {
            let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
            assert!(
                c.web.endereco().unwrap().ip().is_loopback(),
                "a porta web nasceu aberta a rede em {:?} ({txt})",
                c.web.bind
            );
            assert!(
                !escuta_fora_da_maquina(&c.web.bind),
                "{txt}: {:?} atende de fora",
                c.web.bind
            );
        }
        // O irmao: as duas portas do REST nascem no mesmo lugar, pelo mesmo
        // motivo. Elas ja nasciam -- o que faltava era alguem travar isso.
        let c = Config::de_json(&Json::analisar(r#"{"token":"x"}"#).unwrap()).unwrap();
        assert!(!escuta_fora_da_maquina(&c.rest.bind));
        assert!(!escuta_fora_da_maquina(&c.rest.swagger_bind));
    }

    /// `0.0.0.0` e o caso que `is_loopback` sozinho deixaria passar: nao e
    /// laco local e nao e endereco de placa -- e TODA placa, o mais exposto.
    #[test]
    fn o_nao_especificado_conta_como_porta_aberta_ao_mundo() {
        assert!(escuta_fora_da_maquina("0.0.0.0:5001"));
        assert!(escuta_fora_da_maquina("[::]:5001"));
        assert!(escuta_fora_da_maquina("10.0.0.7:5001"));
        assert!(!escuta_fora_da_maquina("127.0.0.1:5001"));
        assert!(!escuta_fora_da_maquina("[::1]:5001"));
        // Endereco que nao resolve nao vira aviso: quem o recusa e o
        // `validar`, e dois textos sobre a mesma linha errada confundem.
        assert!(!escuta_fora_da_maquina("isso nao e endereco"));
    }

    /// Abrir a porta web ao mundo continua PODENDO -- e passa a avisar.
    ///
    /// Recusar derrubaria toda instalacao que hoje termina TLS num proxy e
    /// faz a coisa certa. O aviso diz o que fazer, e nao so o que esta ruim.
    #[test]
    fn porta_http_aberta_ao_mundo_avisa_no_arranque_e_sobe() {
        let txt = r#"{"token":"x","web":{"ligado":true,"bind":"0.0.0.0:8080"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        c.validar().unwrap();
        let aviso = c
            .avisos
            .iter()
            .find(|a| a.starts_with("web.bind"))
            .unwrap_or_else(|| panic!("nenhum aviso sobre a porta aberta: {:?}", c.avisos));
        // O aviso tem de DIZER O QUE FAZER. Aviso que so reclama vira ruido.
        assert!(aviso.contains("proxy"), "{aviso}");
        assert!(aviso.contains("TLS"), "{aviso}");
        assert!(aviso.contains("127.0.0.1"), "{aviso}");
        assert!(aviso.contains("atras_de_proxy"), "{aviso}");
    }

    /// O irmao: as duas portas do REST avisam pelo mesmo caminho.
    ///
    /// `rest.bind` e `rest.swagger_bind` chamam `endereco()` e
    /// `TcpListener::bind` no arranque exatamente como a web -- irmao e quem
    /// chama as mesmas funcoes na mesma ordem.
    #[test]
    fn as_portas_do_rest_avisam_pelo_mesmo_caminho() {
        let txt = r#"{"token":"x","rest":{"ligado":true,"bind":"0.0.0.0:6000",
            "swagger_ligado":true,"swagger_bind":"0.0.0.0:7000"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(
            c.avisos.iter().any(|a| a.starts_with("rest.bind")),
            "{:?}",
            c.avisos
        );
        assert!(
            c.avisos.iter().any(|a| a.starts_with("rest.swagger_bind")),
            "{:?}",
            c.avisos
        );
    }

    /// Declarar o proxy cala o aviso -- e essa e a razao de o campo existir.
    ///
    /// Aviso que aparece para sempre em instalacao correta e aviso que
    /// ninguem le, e isso gasta a confianca do aviso verdadeiro.
    #[test]
    fn proxy_declarado_cala_o_aviso_da_porta_aberta() {
        let txt = r#"{"token":"x","web":{"ligado":true,"bind":"0.0.0.0:8080",
            "atras_de_proxy":true}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.web.atras_de_proxy);
        assert!(
            !c.avisos.iter().any(|a| a.starts_with("web.bind")),
            "o proxy foi declarado e o aviso saiu assim mesmo: {:?}",
            c.avisos
        );
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    /// **O comportamento velho, e e o teste que mais importa aqui.**
    ///
    /// Uma instalacao correta -- as tres portas HTTP onde elas sempre
    /// estiveram, e sem exigir a cifra do fio -- nao ganha aviso nenhum.
    /// Aviso falso e do mesmo naipe do campo que mente: gasta a confianca do
    /// que e verdadeiro, e aviso que sempre aparece ninguem le.
    ///
    /// O `"exigir": false` esta escrito, e nao omitido, de proposito: este
    /// teste e sobre o aviso das PORTAS, e escrever o campo o deixa dizendo a
    /// mesma coisa no dia em que o padrao do `exigir` virar. Quem trava o
    /// padrao e o `sem_a_secao_cifra_fio_a_cifra_ja_e_exigida`, e ele sozinho --
    /// dois testes travando a mesma coisa viram dois lugares onde a lei pode
    /// divergir de si mesma.
    #[test]
    fn config_de_ontem_no_laco_local_nao_ganha_aviso_nenhum() {
        let txt = r#"{"token":"x","cifra_fio":{"exigir":false},
            "web":{"ligado":true},
            "rest":{"ligado":true,"swagger_ligado":true}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    /// Porta DESLIGADA nao avisa: ninguem escuta nela.
    ///
    /// O `web.bind` continua no arquivo (e continua valendo no dia em que
    /// alguem ligar), mas avisar sobre porta fechada e o mesmo aviso falso.
    #[test]
    fn porta_desligada_com_endereco_aberto_nao_avisa() {
        let txt = r#"{"token":"x","web":{"ligado":false,"bind":"0.0.0.0:8080"},
            "rest":{"ligado":false,"bind":"0.0.0.0:6000"}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
    }

    /// **O aviso do `cifra_fio.exigir`, e ele MUDOU DE ASSUNTO no pedido 370.**
    ///
    /// Ate 18/09/2026 este teste travava o ALCANCE: o `exigir` era lido num
    /// lugar so que decidia algo -- o laco da porta de dados --, e no MESMO
    /// servidor e no MESMO instante `POST /api {"op":"login"}` devolvia 200
    /// com a sessao aberta e a senha em claro. Enquanto as portas HTTP nao
    /// recusassem, a saida honesta era o servidor DIZER o alcance.
    ///
    /// **Elas passaram a recusar**, e o teste nao foi apagado: o assunto do
    /// aviso virou a CONSEQUENCIA. Com a exigencia ligada -- que agora e o
    /// padrao --, uma porta HTTP sem proxy declarado nao atende mais nada, e
    /// quem so trocou o binario veria a tela morrer sem saber por que. O aviso
    /// nomeia as portas e as duas saidas escritas.
    #[test]
    fn exigir_a_cifra_do_fio_com_porta_http_sem_proxy_avisa_que_ela_recusa() {
        let txt = r#"{"token":"x","cifra_fio":{"exigir":true},
            "web":{"ligado":true},"rest":{"ligado":true}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let aviso = c
            .avisos
            .iter()
            .find(|a| a.starts_with("cifra_fio.exigir"))
            .unwrap_or_else(|| panic!("o alcance ficou calado: {:?}", c.avisos));
        // Nomeia as portas que vao recusar -- lista generica nao ensina qual
        // declarar.
        assert!(aviso.contains("web.bind"), "{aviso}");
        assert!(aviso.contains("rest.bind"), "{aviso}");
        // E diz as DUAS saidas escritas, senao o aviso vira um beco.
        assert!(aviso.contains("atras_de_proxy"), "{aviso}");
        assert!(aviso.contains("RECUSAM"), "{aviso}");
        // O alcance INBOUND-ONLY saiu daqui de proposito: ele virou aviso
        // proprio, que so sai quando ha saida CONFIGURADA em claro -- ver
        // `exigir_com_saida_em_claro_avisa_que_o_outro_lado_vai_recusar`.
        // Preso a este, ele apareceria em toda instalacao com tela, inclusive
        // nas que nao conectam em lugar nenhum, e aviso que aparece sempre
        // ninguem le.
        assert!(
            !aviso.contains("cluster.cifra"),
            "o alcance inbound-only voltou a viajar no aviso das portas: \
             {aviso}"
        );
    }

    /// **O outro sentido: quem declarou o proxy nao ganha aviso nenhum.**
    ///
    /// Aviso que aparece para sempre numa instalacao correta e aviso que
    /// ninguem le, e isso gasta a confianca do aviso verdadeiro. Com
    /// `exigir` nascendo ligado, sem esta guarda o aviso sairia em TODA
    /// instalacao com tela -- que e a instalacao normal.
    #[test]
    fn exigir_com_o_proxy_declarado_nao_avisa_nada() {
        let txt = r#"{"token":"x","cifra_fio":{"exigir":true},
            "web":{"ligado":true,"atras_de_proxy":true},
            "rest":{"ligado":true,"swagger_ligado":true,"atras_de_proxy":true}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
        assert!(c.estranhas.is_empty(), "{:?}", c.estranhas);
    }

    /// **O aviso das SAIDAS, e ele mudou de assunto com a virada.**
    ///
    /// Ate 18/09/2026 este teste se chamava
    /// `exigir_com_saida_em_claro_avisa_que_o_outro_lado_vai_recusar` e travava
    /// o contrario: a saida em CLARO era o esquecimento, e o aviso nomeava o
    /// esquecimento. Com o padrao de saida ligado, claro so existe escrito --
    /// e avisar quem escreveu o escape e avisar contra a decisao dele, todo
    /// arranque.
    ///
    /// O que sobrou de consequencia e o que o aviso diz agora: esta saida vai
    /// PEDIR o aperto, e um PhxSql anterior a essa data nao o atende. O
    /// servidor nao sabe a versao do outro lado, entao fala com quem nao
    /// escreveu decisao nenhuma -- que e quem a virada pegou de surpresa.
    #[test]
    fn saida_no_padrao_de_fabrica_avisa_que_vai_pedir_o_aperto() {
        let txt = r#"{"token":"x","replicacao":{"papel":"replica","origens":[
              {"nome":"matriz","host":"10.0.0.9","porta":5000}]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let aviso = c
            .avisos
            .iter()
            .find(|a| a.contains("PEDIR o aperto"))
            .unwrap_or_else(|| panic!("a saida de fabrica ficou calada: {:?}", c.avisos));
        // Nomeia a saida E o source: uma lista de interruptores nao diz qual
        // conexao vai parar.
        assert!(aviso.contains("replicacao.origens"), "{aviso}");
        assert!(aviso.contains("matriz"), "{aviso}");
        assert!(aviso.contains("10.0.0.9:5000"), "{aviso}");
        // E diz as DUAS saidas escritas, senao o aviso vira um beco.
        assert!(aviso.contains("\"cifra\": false"), "{aviso}");
        assert!(aviso.contains("\"cifra\": true"), "{aviso}");
    }

    /// **O outro sentido, e ele e o que faz o de cima significar alguma coisa:
    /// as DUAS decisoes escritas calam o aviso.**
    ///
    /// `true` e `false` calam, porque o que se cobra e a decisao registrada e
    /// nao um dos valores -- quem escreveu o escape para falar com um source
    /// antigo nao pode ouvir todo arranque que devia ter feito outra coisa. E
    /// um servidor isolado nao ganha nada: sem saida configurada nao ha
    /// conexao que possa parar.
    #[test]
    fn as_duas_decisoes_escritas_calam_o_aviso_da_saida() {
        for escrito in ["true", "false"] {
            let txt = format!(
                r#"{{"token":"x","replicacao":{{"papel":"replica","origens":[
                  {{"nome":"matriz","host":"10.0.0.9","porta":5000,"cifra":{escrito}}}]}}}}"#
            );
            let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
            assert!(
                !c.avisos.iter().any(|a| a.contains("PEDIR o aperto")),
                "\"cifra\": {escrito} nao calou o aviso: {:?}",
                c.avisos
            );
        }
        let isolado = r#"{"token":"x","cifra_fio":{"exigir":true}}"#;
        let c = Config::de_json(&Json::analisar(isolado).unwrap()).unwrap();
        assert!(c.avisos.is_empty(), "{:?}", c.avisos);
    }

    /// **A origem nasce CIFRADA, e o escape escrito e o comportamento velho.**
    ///
    /// O par que trava a virada do 18/09/2026 do lado da replicacao. Sem o
    /// primeiro, uma replica de fabrica continuaria falando claro com um
    /// source que exige de fabrica -- os dois padroes juntos parando a
    /// replicacao com a suite verde, que foi o achado do pedido 370.
    #[test]
    fn a_origem_nasce_cifrada_e_o_escape_escrito_a_deixa_em_claro() {
        let sem_campo = r#"{"token":"x","replicacao":{"papel":"replica","origens":[
              {"nome":"matriz","host":"10.0.0.9","porta":5000}]}}"#;
        let c = Config::de_json(&Json::analisar(sem_campo).unwrap()).unwrap();
        assert!(
            c.replicacao.origens[0].cifra,
            "a origem de fabrica ficou falando em claro"
        );
        // O pino nao vem de brinde: cifra de fabrica protege da escuta
        // passiva, e o pino e afirmacao sobre a identidade do outro lado.
        assert!(c.replicacao.origens[0].chave_do_fio.is_empty());

        let escape = sem_campo.replace(r#""porta":5000}"#, r#""porta":5000,"cifra":false}"#);
        let c = Config::de_json(&Json::analisar(&escape).unwrap()).unwrap();
        assert!(
            !c.replicacao.origens[0].cifra,
            "o escape escrito nao foi obedecido"
        );
    }

    /// **Valor TORTO nao rebaixa -- e esta armadilha so nasceu com a virada.**
    ///
    /// Com o padrao desligado, "valor que nao entendi vira `false`" era
    /// inofensivo: dava no mesmo que a ausencia. Com o padrao ligado, o mesmo
    /// caminho viraria um REBAIXAMENTO silencioso da cifra por causa de um
    /// engano de digitacao -- a armadilha que o pedido 373 pagou no ODBC, onde
    /// `CIFRA=sim` cairia em claro.
    ///
    /// Os TRES estados, no mesmo teste, porque e a distincao entre eles que e
    /// a garantia: ausente (padrao, avisa), escrito (obedece, cala) e torto
    /// (padrao, e avisa dizendo o que corrigir).
    #[test]
    fn valor_torto_no_cifra_de_saida_nao_rebaixa_e_avisa() {
        let txt = r#"{"token":"x","replicacao":{"papel":"replica","origens":[
              {"nome":"matriz","host":"10.0.0.9","porta":5000,"cifra":"sim"}]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(
            c.replicacao.origens[0].cifra,
            "valor torto DESLIGOU a cifra: e o rebaixamento silencioso"
        );
        let aviso = c
            .avisos
            .iter()
            .find(|a| a.contains("nao e true nem false"))
            .unwrap_or_else(|| panic!("o valor torto passou calado: {:?}", c.avisos));
        assert!(aviso.contains("matriz"), "{aviso}");
        // E o torto NAO entra no aviso do padrao de fabrica: ele nao e
        // omissao, e um campo escrito errado -- dizer as duas coisas da mesma
        // saida no mesmo arranque e o comeco do aviso que ninguem le.
        assert!(
            !c.avisos.iter().any(|a| a.contains("PEDIR o aperto")),
            "{:?}",
            c.avisos
        );
    }

    /// Sem porta HTTP no ar, o `exigir` cobre o que ha -- e nao avisa nada.
    ///
    /// O outro sentido da prova: um aviso que sai sempre nao distingue o
    /// servidor que tem o furo do servidor que nao tem.
    #[test]
    fn exigir_sem_porta_http_no_ar_nao_avisa_alcance_nenhum() {
        let txt = r#"{"token":"x","cifra_fio":{"exigir":true}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(
            !c.avisos.iter().any(|a| a.starts_with("cifra_fio.exigir")),
            "{:?}",
            c.avisos
        );
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

    /// **O texto solto TAMBEM virou (18/09/2026)**, e este teste se chamava
    /// `web_servidores_texto_solto_continua_em_claro`.
    ///
    /// Deixa-lo em claro seria a virada nao alcancar a forma mais escrita das
    /// duas: quem lista `"host:porta"` nao esta escolhendo o claro, esta
    /// escrevendo o endereco. O escape existe e e o objeto com
    /// `"cifra": false` -- ver `o_escape_escrito_deixa_o_destino_da_tela_em_claro`.
    ///
    /// O PINO continua nao nascendo, pelo mesmo motivo do cluster: o texto
    /// solto nao tem onde carregar um, e padrao nenhum afirma identidade.
    #[test]
    fn web_servidores_texto_solto_passa_a_pedir_o_aperto() {
        let txt = r#"{"token":"x","web":{"servidores":["10.1.1.5:5000","curitiba:5000"]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.web.servidores.len(), 2);
        for s in &c.web.servidores {
            assert!(
                s.cifra,
                "texto solto ficou falando em claro: {}",
                s.endereco
            );
            assert!(s.chave_do_fio.is_empty(), "texto solto nao tem pino");
            assert!(s.pino_do_fio().unwrap().is_none());
        }
        assert_eq!(c.web.servidores[0].endereco, "10.1.1.5:5000");
        c.validar().unwrap();
    }

    /// **O COMPORTAMENTO VELHO da tela, pelo escape ESCRITO.** Um destino que
    /// e um PhxSql anterior ao aperto vira objeto com `"cifra": false`, e a
    /// interface volta a falar claro com ele -- como antes da virada.
    #[test]
    fn o_escape_escrito_deixa_o_destino_da_tela_em_claro() {
        let txt = r#"{"token":"x","web":{"servidores":[
            {"host":"antigo","porta":5000,"cifra":false}
        ]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.web.servidores[0].endereco, "antigo:5000");
        assert!(
            !c.web.servidores[0].cifra,
            "o escape escrito nao foi obedecido"
        );
    }

    /// FORMATO NOVO: o objeto `{host,porta,cifra,chave_do_fio}` carrega o pino,
    /// e o endereco sai remontado como "host:porta" para casar o destino.
    #[test]
    fn web_servidores_aceita_objeto_com_cifra_e_pino() {
        let pino = "aa".repeat(32);
        let txt = format!(
            r#"{{"token":"x","web":{{"servidores":[
                {{"host":"10.0.0.9","porta":5000,"cifra":true,"chave_do_fio":"{pino}"}}
            ]}}}}"#
        );
        let c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let s = &c.web.servidores[0];
        assert_eq!(s.endereco, "10.0.0.9:5000");
        assert!(s.cifra);
        assert_eq!(s.pino_do_fio().unwrap(), Some([0xaau8; 32]));
        // E o endereco remontado casa o destino, igual ao texto solto.
        assert!(c.web.servidor_permitido("10.0.0.9:5000"));
        assert!(c.web.servidor("10.0.0.9:5000").unwrap().cifra);
        c.validar().unwrap();
    }

    /// As DUAS formas na mesma lista -- so o item que precisa do PINO (ou do
    /// escape) vira objeto, o resto fica texto.
    #[test]
    fn web_servidores_mistura_texto_e_objeto() {
        let txt = r#"{"token":"x","web":{"servidores":[
            "claro:5000",
            {"host":"cifrado","porta":6000,"cifra":true,"chave_do_fio":"bb"}
        ]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert_eq!(c.web.servidores.len(), 2);
        // O texto solto do meio da lista continua sendo aceito -- o que mudou
        // com a virada de 18/09/2026 e o que ele significa: pede o aperto,
        // como o objeto ao lado. Quem quer claro escreve o objeto com
        // `"cifra": false`.
        assert_eq!(c.web.servidores[0].endereco, "claro:5000");
        assert!(c.web.servidores[0].cifra);
        assert_eq!(c.web.servidores[1].endereco, "cifrado:6000");
        assert!(c.web.servidores[1].cifra);
    }

    /// O OBJETO sem o campo `cifra` nasce cifrado como o texto solto -- as
    /// duas formas da mesma lista tem de dar o mesmo destino, senao a forma
    /// escolhida para escrever o endereco decidiria a seguranca dele.
    #[test]
    fn web_servidor_em_objeto_sem_o_campo_cifra_nasce_cifrado() {
        let txt = r#"{"token":"x","web":{"servidores":[{"host":"h","porta":5000}]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        assert!(
            c.web.servidores[0].cifra,
            "o objeto sem o campo ficou falando em claro"
        );
    }

    /// `cifra:true` SEM pino = tunel so passivo, e o pino vem `None` de
    /// proposito (nao um erro): e o caso legitimo que o arranque AVISA. O que
    /// nao pode e virar `None` calado por pino torto -- esse e o proximo teste.
    #[test]
    fn web_servidor_com_cifra_sem_pino_e_passivo() {
        let txt = r#"{"token":"x","web":{"servidores":[
            {"host":"h","porta":5000,"cifra":true}
        ]}}"#;
        let c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
        let s = &c.web.servidores[0];
        assert!(s.cifra);
        assert!(s.chave_do_fio.is_empty());
        assert!(
            s.pino_do_fio().unwrap().is_none(),
            "sem pino e None, nao erro"
        );
        c.validar().unwrap();
    }

    /// Pino TORTO vira erro no arranque, e nao um tunel sem pino calado -- a
    /// mesma disciplina que `pino_torto_na_origem_e_erro_e_nao_ausencia` trava
    /// para a replicacao. Um pino escrito errado que virasse `None` seria
    /// exatamente o estrago que o pino existe para impedir.
    #[test]
    fn web_pino_torto_e_erro_no_arranque() {
        let mau = r#"{"token":"x","web":{"servidores":[
            {"host":"h","porta":5000,"cifra":true,"chave_do_fio":"abacaxi"}
        ]}}"#;
        let c = Config::de_json(&Json::analisar(mau).unwrap()).unwrap();
        assert!(
            c.validar().is_err(),
            "hex torto tinha de reprovar o arranque"
        );

        let curto = r#"{"token":"x","web":{"servidores":[
            {"host":"h","porta":5000,"cifra":true,"chave_do_fio":"aaaa"}
        ]}}"#;
        let c = Config::de_json(&Json::analisar(curto).unwrap()).unwrap();
        assert!(c.validar().is_err(), "2 bytes passaram por 32");
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

    /// O teto das threads HTTP (pedido 248): nasce em 64 com fila de 2 s,
    /// le do `recursos`, e zero e uma escolha valida nos dois -- sem teto e
    /// sem espera --, nao um erro que caia no padrao.
    #[test]
    fn o_teto_da_web_nasce_em_64_com_fila_de_dois_segundos_e_le_do_config() {
        let c = cfg(r#"{"token":"t"}"#);
        assert_eq!(c.recursos.conexoes_web_max, 64);
        assert_eq!(c.recursos.fila_web_ms, 2_000);

        let c = cfg(r#"{"token":"t","recursos":{"conexoes_web_max":8,"fila_web_ms":150}}"#);
        assert_eq!(c.recursos.conexoes_web_max, 8);
        assert_eq!(c.recursos.fila_web_ms, 150);

        let c = cfg(r#"{"token":"t","recursos":{"conexoes_web_max":0,"fila_web_ms":0}}"#);
        assert_eq!(
            c.recursos.conexoes_web_max, 0,
            "zero = sem teto, e fica zero"
        );
        assert_eq!(c.recursos.fila_web_ms, 0, "zero = nao espere, e fica zero");

        // Os dois saem no `config` que a tela le, e a secao os conhece: um
        // campo que o servidor le e sobre o qual ele avisa «desconhecido»
        // gasta a confianca do aviso verdadeiro.
        let j = c.recursos.para_json();
        assert!(j.campo("conexoes_web_max").is_some());
        assert!(j.campo("fila_web_ms").is_some());
        let (_, lista) = SECOES_CONHECIDAS
            .iter()
            .find(|(s, _)| *s == "recursos")
            .unwrap();
        assert!(lista.contains(&"conexoes_web_max"));
        assert!(lista.contains(&"fila_web_ms"));
    }

    /// Configuracao que nao e lida mente: cada campo de `alertas.disco` e de
    /// `alertas.sms` tem leitor, e o valor lido e o do arquivo, nao o padrao.
    /// Reponha o defeito trocando `d.inteiro_ou("checar_segundos", ..)` pelo
    /// padrao e este teste cai no primeiro `assert_eq`.
    #[test]
    fn alertas_disco_e_sms_sao_lidos_do_arquivo() {
        let c = Config::de_json(
            &Json::analisar(
                r#"{"token":"t","alertas":{
                    "disco":{"ligado":false,"checar_segundos":7,"repetir_minutos":3,"lento_ms":250},
                    "email":{"ligado":true,"servidor":"127.0.0.1","de":"a@b.c","para":["x@y.z"]},
                    "sms":{"ligado":true,"numeros":["+5541999990000","41988887777"],"gateway_email":"sms.op.com.br"}
                }}"#,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!c.alertas.disco.ligado);
        assert_eq!(c.alertas.disco.checar_segundos, 7);
        assert_eq!(c.alertas.disco.repetir_minutos, 3);
        assert_eq!(c.alertas.disco.lento_ms, 250);
        assert!(c.alertas.sms.ligado);
        assert_eq!(
            c.alertas.sms.enderecos(),
            vec!["+5541999990000@sms.op.com.br", "41988887777@sms.op.com.br"]
        );
        // Os dois saem no `config` que a tela le, e a secao os conhece.
        let j = c.alertas.para_json();
        assert_eq!(
            j.campo("disco")
                .and_then(|d| d.campo("checar_segundos"))
                .and_then(Json::numero),
            Some(7.0)
        );
        assert_eq!(
            j.campo("sms")
                .and_then(|d| d.campo("gateway_email"))
                .and_then(Json::texto),
            Some("sms.op.com.br")
        );
        let (_, lista) = SECOES_CONHECIDAS
            .iter()
            .find(|(s, _)| *s == "alertas")
            .unwrap();
        assert!(lista.contains(&"disco") && lista.contains(&"sms"));
        assert!(SECOES_CONHECIDAS.iter().any(|(s, _)| *s == "alertas.disco"));
        assert!(SECOES_CONHECIDAS.iter().any(|(s, _)| *s == "alertas.sms"));
    }

    /// Sem o bloco, a sonda nasce LIGADA a cada 60 s -- e sem SMS. E o
    /// comportamento de quem nunca abriu o config.json: o painel mede.
    #[test]
    fn sem_o_bloco_a_sonda_nasce_ligada_e_o_sms_desligado() {
        let c = Config::de_json(&Json::analisar(r#"{"token":"t"}"#).unwrap()).unwrap();
        assert!(c.alertas.disco.ligado);
        assert_eq!(c.alertas.disco.checar_segundos, 60);
        assert_eq!(c.alertas.disco.repetir_minutos, 30);
        assert_eq!(c.alertas.disco.lento_ms, 1_000);
        assert!(!c.alertas.sms.ligado);
        assert!(c.alertas.sms.enderecos().is_empty());
        // Zero segundos nao existe: a sonda em laco apertado seria ela mesma
        // o problema de disco.
        let c = Config::de_json(
            &Json::analisar(r#"{"token":"t","alertas":{"disco":{"checar_segundos":0}}}"#).unwrap(),
        )
        .unwrap();
        assert_eq!(c.alertas.disco.checar_segundos, 1);
    }

    /// O SMS recusa no ARRANQUE o que nao conseguiria entregar: sem e-mail
    /// ligado, sem numero, numero que nao e numero, gateway com arroba.
    #[test]
    fn o_sms_recusa_no_arranque_o_que_nao_entregaria() {
        let erro = |json: &str| {
            Config::de_json(&Json::analisar(json).unwrap())
                .err()
                .map(|e| e.to_string())
                .unwrap_or_default()
        };
        let email =
            r#""email":{"ligado":true,"servidor":"127.0.0.1","de":"a@b.c","para":["x@y.z"]}"#;
        let e = erro(
            r#"{"token":"t","alertas":{"sms":{"ligado":true,"numeros":["41999990000"],"gateway_email":"sms.op"}}}"#,
        );
        assert!(e.contains("sem alertas.email ligado"), "{e}");
        let e = erro(&format!(
            r#"{{"token":"t","alertas":{{{email},"sms":{{"ligado":true,"gateway_email":"sms.op"}}}}}}"#
        ));
        assert!(e.contains("sem \"numeros\""), "{e}");
        let e = erro(&format!(
            r#"{{"token":"t","alertas":{{{email},"sms":{{"ligado":true,"numeros":["41 99999-0000"],"gateway_email":"sms.op"}}}}}}"#
        ));
        assert!(e.contains("nao e um numero"), "{e}");
        let e = erro(&format!(
            r#"{{"token":"t","alertas":{{{email},"sms":{{"ligado":true,"numeros":["41999990000"],"gateway_email":"x@sms.op"}}}}}}"#
        ));
        assert!(e.contains("nao e um dominio"), "{e}");
        let e = erro(&format!(
            r#"{{"token":"t","alertas":{{{email},"sms":{{"ligado":true,"numeros":["41999990000"],"gateway_email":"sms.op"}}}}}}"#
        ));
        assert!(e.is_empty(), "configuracao valida recusada: {e}");
        // Desligado, nada se confere: campo em branco nao pode derrubar o
        // arranque de quem nao usa SMS.
        let e = erro(r#"{"token":"t","alertas":{"sms":{"ligado":false,"numeros":["abc"]}}}"#);
        assert!(e.is_empty(), "{e}");
    }

    /// `threads` e `cpu_percentual` tem leitor de verdade: o teto global do
    /// trabalho dividido. Antes disto os dois campos existiam no config.json,
    /// no MANUAL e na tela, e `paralelo::nucleos()` perguntava direto a
    /// maquina -- a mesma armadilha do `cache_paginas` sem cache.
    ///
    /// Pedido 234: esta prova era `aplicar()` seguido de
    /// `assert_eq!(phxsql_core::paralelo::nucleos(), 1)` -- e `paralelo::TETO`
    /// e ESTADO GLOBAL DE PROCESSO, nao coisa deste teste. Toda dezena de
    /// outros testes deste arquivo chama `Config::ler`, que chama
    /// `recursos.aplicar()`, que redefine o mesmo global; numa suite paralela
    /// (o padrao do `cargo test`) um vizinho podia redefinir o teto ENTRE o
    /// `aplicar()` e o `assert_eq!` daqui, e o teste caia sem o calculo estar
    /// errado. Medido: 4 quedas em 200 corridas do binario de teste
    /// (`docs/cognicao/cognicao_estado-global-entre-testes-do-mesmo-binario_20260909_0610.md`),
    /// sempre com `left` igual ao numero de nucleos da maquina (o teto que
    /// OUTRO teste tinha acabado de gravar), nunca um numero aleatorio --
    /// prova de que era o vizinho, nao um calculo furado.
    ///
    /// O conserto e por CONTRATO, nao por mutex: este teste prova so o
    /// CALCULO (`Recursos::nucleos()`), sem tocar `phxsql_core::paralelo`; o
    /// global isolado (`definir_teto`/`nucleos()`) tem prova propria em
    /// `phxsql_core::paralelo::tests::o_teto_configurado_vale`, no binario de
    /// teste do `phxsql-core`, onde mais nenhum teste mexe em `TETO`. A linha
    /// que compoe os dois -- `Recursos::aplicar()` chamando
    /// `paralelo::definir_teto(self.nucleos())`, poucas linhas acima -- fica
    /// correta por revisao: cada metade e testada onde ela sozinha decide o
    /// resultado.
    #[test]
    fn threads_e_cpu_viram_o_teto_do_paralelo() {
        // 4 threads a 25% = teto de UM nucleo -- e um e o resultado em
        // qualquer maquina, porque o teto so corta, nunca inventa nucleo.
        let c = cfg(r#"{"token":"t","recursos":{"threads":4,"cpu_percentual":25}}"#);
        assert_eq!(c.recursos.nucleos(), 1, "o calculo do teto nao bateu");
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
        assert_eq!(c.alertas.email.senha().unwrap(), "segredo-do-rele");
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
        assert_eq!(c.alertas.email.senha().unwrap(), "vinda-do-ambiente");
    }

    /// `alertas.email.senha_env` que falta: o envio recusa NOMEANDO a
    /// variavel, antes de ir ao rele -- e o arranque sobe.
    ///
    /// O rele e acessorio, como o DbLink: a falta nao derruba o servidor, vira
    /// aviso no arranque e erro no envio. O rele aponta para a porta 1, onde
    /// nao ha ninguem: um erro de conexao aqui seria a prova de que o portao
    /// ficou para depois da rede. Com o defeito reposto, `senha()` volta
    /// `Ok("")` e a primeira assercao cai.
    #[test]
    fn a_senha_do_rele_que_falta_no_ambiente_e_erro_nomeado() {
        const AUSENTE: &str = "PHXSQL_TESTE_372_SMTP_QUE_NINGUEM_EXPORTA";
        let c = de(&format!(
            r#"{{"token":"x","alertas":{{"ligado":true,"email":{{"ligado":true,
                "servidor":"127.0.0.1","porta":1,"de":"phx@x.com","para":["a@x.com"],
                "usuario":"phx","senha_env":"{AUSENTE}"}}}}}}"#
        ));
        // `match`, e nao `expect_err`: com o defeito reposto o `Ok` traria
        // o valor, e o `expect_err` o imprimiria na saida do teste.
        let e = match c.alertas.email.senha() {
            Ok(_) => panic!("variavel ausente virou senha vazia -- o defeito do 372"),
            Err(e) => e.to_string(),
        };
        assert!(e.contains(AUSENTE) && e.contains("alertas.email"), "{e}");
        let e = crate::email::enviar(&c.alertas.email, "a", "b")
            .unwrap_err()
            .to_string();
        assert!(e.contains(AUSENTE), "o envio foi a rede sem a senha: {e}");
        assert!(
            c.avisos.iter().any(|a| a.contains(AUSENTE)),
            "a falta nao virou aviso: {:?}",
            c.avisos
        );
        assert!(
            c.para_json().escrever().contains("(variavel ausente)"),
            "a tela nao ve a falta"
        );
    }
}

/// A gravacao pela tela: `gravar_campos` e a whitelist.
#[cfg(test)]
mod testes_gravacao {
    use super::*;

    /// Um config.json de verdade no disco, com comentario, ordem propria e um
    /// bloco que este processo nao conhece -- exatamente o que a gravacao nao
    /// pode estragar.
    fn arquivo(nome: &str) -> (DirTemp, PathBuf) {
        let dir = DirTemp::novo(&format!("gravar-{nome}"));
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
        (dir, caminho)
    }

    fn muda(campo: &str, valor: Json) -> Vec<(String, Json)> {
        vec![(campo.to_string(), valor)]
    }

    /// **O `config.json` regravado nasce 0600 -- e nao herda mais o 0644.**
    ///
    /// Antes, o temporario nascia com a permissao do `umask` e depois copiava
    /// a do original: um arquivo de instalacao em 0644, com o token e os
    /// hashes dentro, ficava 0644 para sempre (revisao SEC de 17/09/2026,
    /// achado A4). A fixture e escrita com `std::fs::write` -- 0644 sob
    /// `umask 022`, que e o caso da instalacao --, e a resposta vem do
    /// sistema operacional: o modo que o `stat` devolve.
    #[cfg(unix)]
    #[test]
    fn o_config_regravado_nasce_0600_sem_herdar_o_original() {
        use std::os::unix::fs::PermissionsExt as _;
        let (_guarda, caminho) = arquivo("permissao");
        std::fs::set_permissions(&caminho, std::fs::Permissions::from_mode(0o644)).unwrap();
        let antes = std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777;
        assert_eq!(antes, 0o644, "a fixture nao reproduziu a instalacao aberta");

        Config::gravar_campos(&caminho, &muda("max_linhas", Json::de_i64(50))).unwrap();

        let depois = std::fs::metadata(&caminho).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            depois, 0o600,
            "o config.json continuou legivel por outros: {depois:o}"
        );
        assert!(
            !temporario_de(&caminho).exists(),
            "o temporario ficou para tras"
        );
    }

    /// O molde em si: caminho novo nasce 0600, e um `.tmp` deixado por uma
    /// gravacao interrompida -- com a permissao daquele dia -- nao contamina
    /// a de hoje, porque `mode` so vale na criacao e o `.tmp` velho sai antes.
    #[cfg(unix)]
    #[test]
    fn gravar_privado_nasce_0600_mesmo_com_temporario_velho_aberto() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = DirTemp::novo("gravar-privado");
        let caminho = dir.join("segredo.json");
        let modo = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;

        gravar_privado(&caminho, b"{\"a\":1}").unwrap();
        assert_eq!(modo(&caminho), 0o600);
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), "{\"a\":1}");

        let tmp = temporario_de(&caminho);
        std::fs::write(&tmp, "lixo de uma gravacao interrompida").unwrap();
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644)).unwrap();
        gravar_privado(&caminho, b"{\"a\":2}").unwrap();
        assert_eq!(modo(&caminho), 0o600, "o .tmp velho contaminou a permissao");
        assert_eq!(std::fs::read_to_string(&caminho).unwrap(), "{\"a\":2}");
        assert!(!tmp.exists());
    }

    #[test]
    fn grava_o_pedido_e_preserva_o_resto() {
        let (_guarda, caminho) = arquivo("preserva");
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
        let dir = DirTemp::novo("bytes");
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
        let (_guarda, caminho) = arquivo("ausente");
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
        let (_guarda, caminho) = arquivo("secao");
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
        let (_guarda, caminho) = arquivo("whitelist");
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
        let (_guarda, caminho) = arquivo("tipo");
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
        let (_guarda, caminho) = arquivo("valida");
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
        let (_guarda, caminho) = arquivo("divergencia");
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
        let (_guarda, caminho) = arquivo("velho");
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
        let (_guarda, caminho) = arquivo("cores");
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
        let (_guarda, caminho) = arquivo("cor-torta");
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
