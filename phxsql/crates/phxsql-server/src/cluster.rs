//! O estado VIVO do cluster: papel, epoca, mapa dos nos e a decisao de
//! eleicao -- pedido 126.
//!
//! # O que mora aqui e o que mora no servidor
//!
//! Aqui mora o que se decide SEM soquete: quem venceria uma eleicao dada uma
//! visao do cluster, quando ha maioria, o papel e a epoca deste no, e o
//! arquivo que os persiste. As threads -- pulso, arbitro e o laco que puxa do
//! master corrente -- moram no `servidor.rs`, porque precisam das entranhas
//! dele (a trava de dados, o cliente de replicacao, o e-mail).
//!
//! # Por que a decisao e uma funcao pura
//!
//! [`vencedor`] recebe a visao e devolve o eleito, sem tocar em relogio nem
//! em soquete. E o unico jeito de o teste de protecao mais importante da
//! bateria -- *sem maioria visivel, NAO promove* -- rodar em microssegundos e
//! falhar na cara de quem remover a conferencia.
//!
//! # Honestidade: isto NAO e Raft
//!
//! Nao ha log replicado por quorum de escrita: o master confirma a escrita
//! sem esperar replica nenhuma. A eleicao por maioria impede DOIS masters
//! duradouros (so uma particao enxerga a maioria dos nos CONFIGURADOS), mas
//! nao impede a perda das ultimas escritas de um master isolado: o que ele
//! aceitou entre o inicio da particao e o momento em que se ve sem maioria
//! nao chegou a ninguem, e morre com o rebaixamento. `docs/CLUSTER.md` diz
//! isso com todas as letras, com o que o operador deve saber.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::config::Cluster;

/// Papel VIVO deste no. Pode divergir do `config.json` depois de uma eleicao
/// -- e e por isso que ele se persiste: um master destronado que reiniciasse
/// pelo config voltaria achando que manda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PapelVivo {
    Master,
    Replica,
}

impl PapelVivo {
    pub fn nome(self) -> &'static str {
        match self {
            PapelVivo::Master => "master",
            PapelVivo::Replica => "replica",
        }
    }

    fn de_texto(s: &str) -> Option<PapelVivo> {
        match s {
            "master" => Some(PapelVivo::Master),
            "replica" => Some(PapelVivo::Replica),
            _ => None,
        }
    }
}

/// O que o ultimo pulso disse de um no.
#[derive(Debug, Clone)]
pub struct PulsoDeNo {
    pub papel: PapelVivo,
    pub epoca: u64,
    pub posicao: u64,
    pub prioridade: i64,
    pub quando_ms: i64,
}

impl PulsoDeNo {
    /// Le um pulso de um JSON -- serve para o pedido que chega e para a
    /// resposta que volta, porque os dois carregam os mesmos cinco campos.
    pub fn de_json(j: &Json) -> Option<(String, PulsoDeNo)> {
        let id = j.texto_ou("id", "").trim().to_string();
        if id.is_empty() {
            return None;
        }
        let papel = PapelVivo::de_texto(j.texto_ou("papel", ""))?;
        Some((
            id,
            PulsoDeNo {
                papel,
                epoca: j.inteiro_ou("epoca", 0).max(0) as u64,
                posicao: j.inteiro_ou("posicao", 0).max(0) as u64,
                prioridade: j.inteiro_ou("prioridade", 0),
                quando_ms: crate::agora_ms(),
            },
        ))
    }
}

/// Um no vivo, do jeito que a eleicao o compara.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidato {
    pub id: String,
    pub posicao: u64,
    pub prioridade: i64,
}

/// Quem vence a eleicao, dada a visao dos VIVOS -- ou ninguem.
///
/// `None` quando os vivos nao passam da metade dos nos configurados, e nao
/// promover e a decisao: metade nao e maioria, senao os dois lados de uma
/// particao ao meio elegeriam um master cada.
///
/// Entre os elegiveis vence a maior posicao do diario (quem menos perdeu),
/// empate quebra pela prioridade (maior ganha) e depois pelo MENOR id -- o
/// ultimo criterio existe so para a resposta ser a mesma em todo no que fizer
/// a mesma conta.
pub fn vencedor(vivos: &[Candidato], total_configurado: usize) -> Option<&Candidato> {
    if vivos.len() * 2 <= total_configurado {
        return None;
    }
    vivos.iter().max_by(|a, b| {
        a.posicao
            .cmp(&b.posicao)
            .then(a.prioridade.cmp(&b.prioridade))
            // Invertido de proposito: no empate total, o id MENOR ganha.
            .then_with(|| b.id.cmp(&a.id))
    })
}

const PAPEL_MASTER: u8 = 1;
const PAPEL_REPLICA: u8 = 0;

/// O estado compartilhado entre as threads do cluster e o portao de escrita.
///
/// # Por que atomos no caminho quente
///
/// O portao de escrita consulta papel e maioria em TODO pedido que grava.
/// Instrumentacao desligada custa zero (sem cluster o `Option` e `None`); com
/// cluster ligado, o custo e ler dois atomos -- o mutex do mapa so entra nas
/// threads de fundo e no caminho de ERRO, onde montar a mensagem ja e o
/// trabalho.
pub struct EstadoCluster {
    pub config: Cluster,
    papel: AtomicU8,
    epoca: AtomicU64,
    /// Posicao local do diario, somada sobre as tabelas replicadas. Cache
    /// atualizado pelo arbitro para o pulso nao tomar a trava de dados.
    posicao: AtomicU64,
    /// Master COM maioria visivel = escrita liberada. Quem atualiza e o
    /// arbitro; o portao so le.
    escrita_liberada: AtomicBool,
    /// Ultima vez que um master LEGITIMO (epoca >= a nossa) deu sinal.
    master_visto_ms: AtomicI64,
    /// Id do master corrente, na visao deste no.
    master_id: Mutex<Option<String>>,
    nos: Mutex<HashMap<String, PulsoDeNo>>,
    /// Motivos pelos quais o cluster esta degradado AGORA, para a op
    /// `cluster_estado` e para o e-mail repetido.
    degradado: Mutex<Vec<String>>,
    ultimo_email_ms: AtomicI64,
    /// Aviso de promocao pendente -- sai UMA vez, e por isso e um `take`.
    promocao_a_avisar: Mutex<Option<String>>,
    caminho_estado: PathBuf,
    /// Quando este estado nasceu. O arbitro da UMA janela de graca a partir
    /// daqui: no arranque o primeiro tique roda antes do primeiro pulso, e
    /// sem a graca um cluster perfeitamente sao nasceria "degradado" -- com
    /// e-mail e tudo.
    nascido_ms: i64,
}

impl EstadoCluster {
    /// Levanta o estado: o arquivo persistido ganha do `config.json`, porque
    /// ele conta o que aconteceu DEPOIS do config ser escrito -- um master
    /// destronado que reiniciasse pelo papel do config voltaria mandando.
    pub fn novo(
        config: Cluster,
        base: &Path,
        papel_do_config: crate::config::Papel,
    ) -> EstadoCluster {
        let caminho_estado = base.join("cluster.estado.json");
        let mut papel = match papel_do_config {
            crate::config::Papel::Source => PapelVivo::Master,
            _ => PapelVivo::Replica,
        };
        let mut epoca = 0u64;
        if let Ok(texto) = std::fs::read_to_string(&caminho_estado) {
            if let Ok(j) = Json::analisar(&texto) {
                if let Some(p) = PapelVivo::de_texto(j.texto_ou("papel", "")) {
                    papel = p;
                    epoca = j.inteiro_ou("epoca", 0).max(0) as u64;
                    eprintln!(
                        "cluster: estado retomado de {}: papel {}, epoca {epoca}",
                        caminho_estado.display(),
                        papel.nome()
                    );
                }
            }
        }
        let agora = crate::agora_ms();
        EstadoCluster {
            config,
            papel: AtomicU8::new(if papel == PapelVivo::Master {
                PAPEL_MASTER
            } else {
                PAPEL_REPLICA
            }),
            epoca: AtomicU64::new(epoca),
            posicao: AtomicU64::new(0),
            // Nasce liberada para o master nao recusar escrita no arranque,
            // antes do primeiro pulso: a primeira rodada do arbitro corrige.
            escrita_liberada: AtomicBool::new(papel == PapelVivo::Master),
            // O arranque vale como "acabei de ver o master": da ao cluster a
            // janela inteira para se apresentar antes de alguem abrir eleicao.
            master_visto_ms: AtomicI64::new(agora),
            master_id: Mutex::new(None),
            nos: Mutex::new(HashMap::new()),
            degradado: Mutex::new(Vec::new()),
            ultimo_email_ms: AtomicI64::new(0),
            promocao_a_avisar: Mutex::new(None),
            caminho_estado,
            nascido_ms: agora,
        }
    }

    pub fn nascido_ms(&self) -> i64 {
        self.nascido_ms
    }

    pub fn papel(&self) -> PapelVivo {
        if self.papel.load(Ordering::SeqCst) == PAPEL_MASTER {
            PapelVivo::Master
        } else {
            PapelVivo::Replica
        }
    }

    pub fn epoca(&self) -> u64 {
        self.epoca.load(Ordering::SeqCst)
    }

    pub fn posicao(&self) -> u64 {
        self.posicao.load(Ordering::SeqCst)
    }

    pub fn definir_posicao(&self, p: u64) {
        self.posicao.store(p, Ordering::SeqCst);
    }

    pub fn escrita_liberada(&self) -> bool {
        self.escrita_liberada.load(Ordering::SeqCst)
    }

    pub fn liberar_escrita(&self, sim: bool) {
        self.escrita_liberada.store(sim, Ordering::SeqCst);
    }

    pub fn master_visto_ms(&self) -> i64 {
        self.master_visto_ms.load(Ordering::SeqCst)
    }

    /// O master corrente na visao deste no: `(id, epoca)`. Um no que E o
    /// master aponta a si mesmo -- sem isso o `cluster_estado` do proprio
    /// master diria "sem master", que e mentira na fonte mais confiavel.
    pub fn master_atual(&self) -> Option<(String, u64)> {
        if self.papel() == PapelVivo::Master {
            return Some((self.config.id.clone(), self.epoca()));
        }
        let id = self.master_id.lock().ok()?.clone()?;
        Some((id, self.epoca()))
    }

    /// Registra o que um pulso disse de um no -- tanto o pedido que chegou
    /// quanto a resposta que voltou passam por aqui.
    ///
    /// Um "master" com epoca menor que a nossa NAO renova o sinal de master:
    /// e o destronado que ainda nao se deu conta, e trata-lo como master
    /// adiaria a eleicao de verdade.
    pub fn registrar(&self, id: &str, pulso: PulsoDeNo) {
        let agora = pulso.quando_ms;
        if pulso.papel == PapelVivo::Master && pulso.epoca >= self.epoca() {
            self.master_visto_ms.store(agora, Ordering::SeqCst);
            // Replica espelha a epoca do master: e assim que "houve eleicao
            // N" atravessa o cluster ate quem nunca falou com o novo master.
            if self.papel() == PapelVivo::Replica && pulso.epoca > self.epoca() {
                self.epoca.store(pulso.epoca, Ordering::SeqCst);
                let _ = self.persistir();
            }
            if let Ok(mut m) = self.master_id.lock() {
                *m = Some(id.to_string());
            }
        }
        if let Ok(mut nos) = self.nos.lock() {
            nos.insert(id.to_string(), pulso);
        }
    }

    /// Copia do mapa, para quem decide ou mostra.
    pub fn mapa(&self) -> HashMap<String, PulsoDeNo> {
        self.nos.lock().map(|m| m.clone()).unwrap_or_default()
    }

    /// Os nos com pulso dentro da janela, ESTE incluido, prontos para a
    /// eleicao.
    pub fn vivos(&self, agora: i64) -> Vec<Candidato> {
        let mut v = vec![Candidato {
            id: self.config.id.clone(),
            posicao: self.posicao(),
            prioridade: self.config.prioridade,
        }];
        for (id, p) in self.mapa() {
            if agora - p.quando_ms <= self.config.janela_ms() {
                v.push(Candidato {
                    id,
                    posicao: p.posicao,
                    prioridade: p.prioridade,
                });
            }
        }
        v
    }

    /// A maior epoca que este no ja viu, a propria incluida.
    pub fn maior_epoca_vista(&self) -> u64 {
        self.mapa()
            .values()
            .map(|p| p.epoca)
            .max()
            .unwrap_or(0)
            .max(self.epoca())
    }

    /// PROMOVE este no a master: epoca nova, papel persistido, escrita
    /// liberada. E o UNICO caminho de promocao -- a eleicao automatica passa
    /// por aqui, e uma promocao manual deve chamar o mesmo lugar, senao os
    /// dois caminhos divergem no primeiro esquecimento.
    pub fn promover(&self, epoca_nova: u64) -> Result<u64> {
        self.epoca.store(epoca_nova, Ordering::SeqCst);
        self.papel.store(PAPEL_MASTER, Ordering::SeqCst);
        self.escrita_liberada.store(true, Ordering::SeqCst);
        if let Ok(mut m) = self.master_id.lock() {
            *m = Some(self.config.id.clone());
        }
        self.master_visto_ms
            .store(crate::agora_ms(), Ordering::SeqCst);
        self.persistir()?;
        Ok(epoca_nova)
    }

    /// REBAIXA este no a replica -- o destronado que viu epoca maior.
    pub fn rebaixar(&self, epoca_do_novo: u64) -> Result<()> {
        self.papel.store(PAPEL_REPLICA, Ordering::SeqCst);
        self.escrita_liberada.store(false, Ordering::SeqCst);
        self.epoca.store(epoca_do_novo, Ordering::SeqCst);
        self.persistir()
    }

    fn persistir(&self) -> Result<()> {
        let texto = Json::objeto(vec![
            ("papel", Json::texto_de(self.papel().nome())),
            ("epoca", Json::de_u64(self.epoca())),
        ])
        .escrever();
        std::fs::write(&self.caminho_estado, texto).map_err(|e| {
            PhxError::Io(std::io::Error::other(format!(
                "{}: {e}",
                self.caminho_estado.display()
            )))
        })
    }

    /// Troca a lista de motivos de degradacao pela atual.
    pub fn definir_degradacao(&self, motivos: Vec<String>) {
        if let Ok(mut d) = self.degradado.lock() {
            *d = motivos;
        }
    }

    pub fn degradacao(&self) -> Vec<String> {
        self.degradado.lock().map(|d| d.clone()).unwrap_or_default()
    }

    /// Agenda o aviso unico de promocao.
    pub fn anotar_promocao(&self, texto: String) {
        if let Ok(mut p) = self.promocao_a_avisar.lock() {
            *p = Some(texto);
        }
    }

    /// Retira o aviso de promocao, se houver -- quem retira, envia.
    pub fn tomar_aviso_de_promocao(&self) -> Option<String> {
        self.promocao_a_avisar.lock().ok()?.take()
    }

    /// Passou o silencio entre dois e-mails de degradacao? Marcar so quando
    /// SIM: quem pergunta e o remetente, e perguntar nao e enviar.
    pub fn hora_de_avisar(&self, agora: i64) -> bool {
        let ultimo = self.ultimo_email_ms.load(Ordering::SeqCst);
        if agora - ultimo >= self.config.avisar_cada_ms() {
            self.ultimo_email_ms.store(agora, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    /// A recusa que o portao de escrita devolve, ou `None` para deixar
    /// passar. So o caminho de ERRO toma o mutex do mapa.
    pub fn recusa_de_escrita(&self) -> Option<PhxError> {
        match self.papel() {
            PapelVivo::Master => {
                if self.escrita_liberada() {
                    None
                } else {
                    Some(PhxError::Autorizacao(format!(
                        "cluster degradado: este master nao enxerga a maioria dos \
                         {} nos configurados; escrita recusada para conter o \
                         split-brain ate a maioria voltar",
                        self.config.nos.len()
                    )))
                }
            }
            PapelVivo::Replica => Some(match self.master_atual() {
                Some((id, epoca)) => match self.config.no(&id) {
                    // Redirecionar para um master CALADO seria apontar um
                    // cadaver: se o ultimo pulso dele ja passou da janela, a
                    // verdade e "eleicao em curso", nao "escreva ali".
                    Some(_)
                        if self.mapa().get(&id).is_some_and(|p| {
                            crate::agora_ms() - p.quando_ms > self.config.janela_ms()
                        }) =>
                    {
                        PhxError::Autorizacao(format!(
                            "este no e replica e o master {id} esta calado alem \
                             da janela (eleicao em curso ou cluster sem \
                             maioria); consulte cluster_estado e tente de novo"
                        ))
                    }
                    Some(no) => PhxError::Redireciona(format!(
                        "REDIRECIONA {} -- este no e replica; o master do \
                         cluster e {id} (epoca {epoca})",
                        no.alvo()
                    )),
                    None => PhxError::Autorizacao(format!(
                        "este no e replica e o master corrente ({id}) nao esta \
                         na lista de nos deste config.json"
                    )),
                },
                None => PhxError::Autorizacao(
                    "este no e replica e ainda nao ha master conhecido \
                     (eleicao em curso ou cluster sem maioria); consulte \
                     cluster_estado e tente de novo"
                        .into(),
                ),
            }),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c(id: &str, posicao: u64, prioridade: i64) -> Candidato {
        Candidato {
            id: id.into(),
            posicao,
            prioridade,
        }
    }

    /// O teste de protecao mais importante da bateria: um no que so enxerga a
    /// si mesmo (ou a metade exata) NAO pode se promover, porque o outro lado
    /// da particao pode estar inteiro e com um master legitimo.
    #[test]
    fn sem_maioria_visivel_nao_promove() {
        // Sozinho entre tres: nao.
        assert!(vencedor(&[c("no3", 999, 9)], 3).is_none());
        // Dois de quatro e METADE, nao maioria: os dois lados de uma particao
        // ao meio fariam a mesma conta e elegeriam um master cada.
        assert!(vencedor(&[c("a", 1, 0), c("b", 1, 0)], 4).is_none());
        // Dois de tres e maioria: promove.
        assert!(vencedor(&[c("a", 1, 0), c("b", 1, 0)], 3).is_some());
    }

    /// Entre os elegiveis vence quem tem MAIS diario -- promover um atrasado
    /// jogaria fora o que os outros ja aplicaram.
    #[test]
    fn com_maioria_vence_a_maior_posicao() {
        let vivos = [c("a", 100, 9), c("b", 250, 0), c("z", 250, 0)];
        assert_eq!(vencedor(&vivos, 3).unwrap().id, "b");
    }

    #[test]
    fn empate_de_posicao_cai_na_prioridade() {
        let vivos = [c("a", 250, 1), c("b", 250, 5)];
        assert_eq!(vencedor(&vivos, 3).unwrap().id, "b");
    }

    /// O ultimo desempate existe para a conta dar IGUAL em todo no.
    #[test]
    fn empate_total_cai_no_menor_id() {
        let vivos = [c("no2", 250, 1), c("no1", 250, 1), c("no3", 250, 1)];
        assert_eq!(vencedor(&vivos, 3).unwrap().id, "no1");
    }

    fn config_de_teste() -> Cluster {
        use crate::config::Config;
        let txt = r#"{"token":"t","replicacao":{"papel":"replica"},
            "cluster":{"id":"no2","prioridade":3,"janela_inatividade_s":4,
              "nos":[{"id":"no1","endereco":"127.0.0.1","porta":5310},
                     {"id":"no2","endereco":"127.0.0.1","porta":5311},
                     {"id":"no3","endereco":"127.0.0.1","porta":5312}]}}"#;
        Config::de_json(&Json::analisar(txt).unwrap())
            .unwrap()
            .cluster
            .unwrap()
    }

    #[test]
    fn o_estado_persiste_e_o_arquivo_ganha_do_config() {
        let dir = std::env::temp_dir().join(format!(
            "phx-cluster-estado-{}-{}",
            std::process::id(),
            crate::agora_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Nasce replica (papel do config) e se promove na epoca 7.
        let e = EstadoCluster::novo(config_de_teste(), &dir, crate::config::Papel::Replica);
        assert_eq!(e.papel(), PapelVivo::Replica);
        e.promover(7).unwrap();
        assert_eq!(e.papel(), PapelVivo::Master);

        // Renasce dizendo-se replica no config -- e o arquivo corrige: quem
        // foi promovido continua master, na mesma epoca.
        let e2 = EstadoCluster::novo(config_de_teste(), &dir, crate::config::Papel::Replica);
        assert_eq!(e2.papel(), PapelVivo::Master);
        assert_eq!(e2.epoca(), 7);

        // E o rebaixado renasce rebaixado, mesmo que o config diga source.
        e2.rebaixar(9).unwrap();
        let e3 = EstadoCluster::novo(config_de_teste(), &dir, crate::config::Papel::Source);
        assert_eq!(e3.papel(), PapelVivo::Replica);
        assert_eq!(e3.epoca(), 9);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A replica redireciona para o master que conhece -- com o pedaco
    /// `REDIRECIONA host:porta` no comeco, que e o que o cliente recorta.
    #[test]
    fn replica_redireciona_para_o_master() {
        let dir = std::env::temp_dir().join(format!(
            "phx-cluster-redir-{}-{}",
            std::process::id(),
            crate::agora_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let e = EstadoCluster::novo(config_de_teste(), &dir, crate::config::Papel::Replica);

        // Sem master conhecido: recusa explicando, sem endereco inventado.
        match e.recusa_de_escrita() {
            Some(PhxError::Autorizacao(m)) => assert!(m.contains("master"), "{m}"),
            outro => panic!("esperava recusa sem master, veio {outro:?}"),
        }

        e.registrar(
            "no1",
            PulsoDeNo {
                papel: PapelVivo::Master,
                epoca: 2,
                posicao: 10,
                prioridade: 0,
                quando_ms: crate::agora_ms(),
            },
        );
        match e.recusa_de_escrita() {
            Some(PhxError::Redireciona(m)) => {
                assert!(m.starts_with("REDIRECIONA 127.0.0.1:5310"), "{m}")
            }
            outro => panic!("esperava REDIRECIONA, veio {outro:?}"),
        }
        // E a epoca do master foi espelhada.
        assert_eq!(e.epoca(), 2);

        // Promovido, escreve; sem maioria confirmada pelo arbitro, recusa.
        e.promover(3).unwrap();
        assert!(e.recusa_de_escrita().is_none());
        e.liberar_escrita(false);
        match e.recusa_de_escrita() {
            Some(PhxError::Autorizacao(m)) => assert!(m.contains("maioria"), "{m}"),
            outro => panic!("esperava recusa por maioria, veio {outro:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Um "master" de epoca velha e o destronado que ainda nao sabe: ele nao
    /// renova o sinal de master nem vira alvo de redirecionamento.
    #[test]
    fn master_de_epoca_velha_nao_conta() {
        let dir = std::env::temp_dir().join(format!(
            "phx-cluster-velho-{}-{}",
            std::process::id(),
            crate::agora_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let e = EstadoCluster::novo(config_de_teste(), &dir, crate::config::Papel::Replica);
        e.registrar(
            "no1",
            PulsoDeNo {
                papel: PapelVivo::Master,
                epoca: 5,
                posicao: 10,
                prioridade: 0,
                quando_ms: crate::agora_ms(),
            },
        );
        assert_eq!(e.master_atual().unwrap().0, "no1");

        // O antigo master (epoca 1) pulsa: continua registrado como no vivo,
        // mas o master corrente segue sendo o da epoca 5.
        e.registrar(
            "no3",
            PulsoDeNo {
                papel: PapelVivo::Master,
                epoca: 1,
                posicao: 99,
                prioridade: 0,
                quando_ms: crate::agora_ms(),
            },
        );
        assert_eq!(e.master_atual().unwrap().0, "no1");
        assert_eq!(e.mapa().len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
