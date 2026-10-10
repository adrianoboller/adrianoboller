//! Pedido 765, fatia P7: o perfil habitual de cada usuario, que SO OBSERVA.
//!
//! Desenho em `docs/propostas/protecao-765-desenho.md` §4.5 (a hipotese c3:
//! c1 e c2, perfil que PROTEGE, morreram medidas -- 98,9% e 40,5% de
//! novidade no M3). Por usuario: quantos pedidos de cada (categoria de op,
//! database, tabela), e o histograma das 24 horas UTC. O desvio vira a
//! ocorrencia amarela `ForaDoPerfil`; nada e recusado, nada muda na resposta.
//!
//! # Quando acusa
//!
//! So depois de o usuario ter **7 dias e 200 pedidos** de historia (antes
//! disso tudo e novo, e o aviso ensinaria a ignorar o aviso), e so para:
//!
//! * **combinacao nunca vista** -- (categoria, database, tabela) que o
//!   usuario nunca pediu. Acusa uma vez: a mesma combinacao entra no perfil
//!   no proprio pedido que a acusou;
//! * **hora rara** -- hora UTC com menos de 1% da massa do usuario. No
//!   maximo uma por hora de relogio, senao a madrugada de quem fez hora extra
//!   viraria cem avisos.
//!
//! Os numeros 7 / 200 / 1% sao RACIOCINADOS, nao medidos (L2 do desenho:
//! falta corpo de trafego real). Estao em constantes para a medida, quando
//! existir, troca-los num lugar so.
//!
//! # A categoria
//!
//! Sai do MESMO motor que decide a permissao ([`Atividade::da_operacao`]),
//! dobrado nas classes do pgaudit. Uma tabela propria de op -> categoria
//! seria a segunda resposta para a pergunta «o que esta op faz», e a que
//! alguem esquecesse de atualizar classificaria a op nova como «outros».
//! Diverge do pgaudit num nome: a classe ROLE vira `administrar`, porque o
//! `Administrar` daqui junta o cadastro de usuarios com a DDL destrutiva
//! (`excluir_tabela`), e chamar isso de «papel» mentiria sobre o que conta.
//!
//! # Nada de dado do usuario
//!
//! O perfil guarda o login, nomes de database e tabela, contagens e horas
//! -- metadado, nunca valor de linha nem texto de SQL. O nome vem cortado em
//! [`TETO_DO_NOME`] bytes: o pedido com tabela inventada de um mega nao
//! vira um mega no arquivo.
//!
//! # O teto
//!
//! [`TETO_DE_USUARIOS`] perfis e [`TETO_DE_COMBINACOES`] combinacoes por
//! perfil. Cheio, a combinacao nova NAO entra -- conta no `coringa` do
//! perfil e continua sendo acusada (uma vez por hora de relogio): quem varia
//! a tabela de proposito nao faz a memoria crescer, e tambem nao some do
//! aviso.
//!
//! # O arquivo
//!
//! `perfis.jsonl`, ao lado do `acessos.log`: uma linha JSON por usuario,
//! reescrita INTEIRA pela troca duravel no maximo uma vez por
//! [`PASSO_DE_GRAVAR_MS`] -- um pedido por linha de arquivo seria um
//! `fsync` por pedido. Uma queda perde no maximo esse passo de aprendizado,
//! que e aviso e nao dado. Formato em `docs/FORMATO.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use crate::usuarios::Atividade;

/// O nome do arquivo, ao lado do `acessos.log`.
pub const NOME_DO_ARQUIVO: &str = "perfis.jsonl";
/// Quantos usuarios a memoria guarda.
pub const TETO_DE_USUARIOS: usize = 1_000;
/// Quantas combinacoes (categoria, database, tabela) cada perfil guarda.
pub const TETO_DE_COMBINACOES: usize = 256;
/// O nome de database ou tabela, cortado aqui.
pub const TETO_DO_NOME: usize = 64;
/// Pedidos de historia antes de o perfil acusar alguma coisa.
pub const PISO_DE_PEDIDOS: u64 = 200;
/// Tempo de historia antes de o perfil acusar alguma coisa.
pub const PISO_DE_HISTORIA_MS: i64 = 7 * 24 * 3_600_000;
/// Hora com menos que esta fracao da massa (em por cento) e rara.
pub const MASSA_RARA_PORCENTO: u64 = 1;
/// De quanto em quanto o arquivo se reescreve.
pub const PASSO_DE_GRAVAR_MS: i64 = 60_000;

const HORA_MS: i64 = 3_600_000;

/// O que o pedido teve de fora do habitual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desvio {
    /// (categoria, database, tabela) nunca pedida por este usuario.
    Combinacao,
    /// Hora UTC com menos de 1% da massa do usuario.
    Hora,
}

impl Desvio {
    pub fn nome(self) -> &'static str {
        match self {
            Desvio::Combinacao => "combinacao_nova",
            Desvio::Hora => "hora_rara",
        }
    }
}

/// A categoria da op, pelo motor da permissao.
pub fn categoria(op: &str) -> &'static str {
    match Atividade::da_operacao(op) {
        Some(Atividade::Ler)
        | Some(Atividade::Diario)
        | Some(Atividade::Verificar)
        | Some(Atividade::Monitorar) => "ler",
        Some(Atividade::Inserir) | Some(Atividade::Alterar) | Some(Atividade::Excluir) => {
            "escrever"
        }
        Some(Atividade::Criar) | Some(Atividade::Reindexar) => "ddl",
        Some(Atividade::Administrar) => "administrar",
        Some(Atividade::Replicar) | None => "outros",
    }
}

#[derive(Debug, Clone, Default)]
struct Perfil {
    primeiro_ms: i64,
    n: u64,
    horas: [u64; 24],
    /// Chave `categoria \x1f database \x1f tabela`: uma `String` por pedido,
    /// e nao tres.
    combinacoes: HashMap<String, u64>,
    coringa: u64,
    /// A hora de relogio (ms / 1 h) do ultimo aviso que tem teto por hora.
    /// So em processo: reiniciar no meio da madrugada custa um aviso a mais.
    ultimo_aviso_por_hora: i64,
}

/// A memoria dos perfis. O servidor a guarda num `Mutex` proprio, fora da
/// trava de dados.
#[derive(Debug, Default)]
pub struct Perfis {
    caminho: Option<PathBuf>,
    mapa: HashMap<String, Perfil>,
    /// Pedidos de usuario que nao coube (o teto de usuarios estava cheio).
    coringa_de_usuarios: u64,
    sujo: bool,
    gravado_ms: i64,
}

fn cortado(s: &str) -> &str {
    if s.len() <= TETO_DO_NOME {
        return s;
    }
    let mut fim = TETO_DO_NOME;
    while !s.is_char_boundary(fim) {
        fim -= 1;
    }
    &s[..fim]
}

fn chave(categoria: &str, database: &str, tabela: &str) -> String {
    let (d, t) = (cortado(database), cortado(tabela));
    let mut k = String::with_capacity(categoria.len() + d.len() + t.len() + 2);
    k.push_str(categoria);
    k.push('\u{1f}');
    k.push_str(d);
    k.push('\u{1f}');
    k.push_str(t);
    k
}

impl Perfis {
    /// Abre a memoria. Arquivo ausente: nasce vazia (nao ha semente -- o
    /// perfil precisa de 7 dias de historia de qualquer jeito). Linha torta
    /// se ignora: o perfil dela se reaprende.
    pub fn abrir(caminho: impl AsRef<Path>, agora_ms: i64) -> Result<Perfis> {
        let caminho = caminho.as_ref().to_path_buf();
        let mut m = Perfis {
            caminho: Some(caminho.clone()),
            gravado_ms: agora_ms,
            ..Perfis::default()
        };
        match std::fs::read_to_string(&caminho) {
            Ok(texto) => {
                for linha in texto.lines().filter(|l| !l.trim().is_empty()) {
                    if m.mapa.len() >= TETO_DE_USUARIOS {
                        break;
                    }
                    if let Some((u, p)) = Json::analisar(linha).ok().as_ref().and_then(de_json) {
                        m.mapa.insert(u, p);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(m)
    }

    /// Um pedido do usuario. Devolve o desvio, quando houve -- julgado
    /// contra o perfil de ANTES deste pedido, que entra nele em seguida.
    pub fn registrar(
        &mut self,
        usuario: &str,
        categoria: &str,
        database: &str,
        tabela: &str,
        agora_ms: i64,
    ) -> Option<Desvio> {
        if usuario.is_empty() {
            return None;
        }
        if !self.mapa.contains_key(usuario) {
            if self.mapa.len() >= TETO_DE_USUARIOS {
                self.coringa_de_usuarios += 1;
                return None;
            }
            self.mapa.insert(
                usuario.to_string(),
                Perfil {
                    primeiro_ms: agora_ms,
                    ultimo_aviso_por_hora: i64::MIN,
                    ..Perfil::default()
                },
            );
        }
        let p = self.mapa.get_mut(usuario)?;
        self.sujo = true;
        let k = chave(categoria, database, tabela);
        let hora = (agora_ms.div_euclid(HORA_MS)).rem_euclid(24) as usize;
        let hora_de_relogio = agora_ms.div_euclid(HORA_MS);
        let maduro = p.n >= PISO_DE_PEDIDOS && agora_ms - p.primeiro_ms >= PISO_DE_HISTORIA_MS;
        let conhecida = p.combinacoes.contains_key(&k);
        let cabe = conhecida || p.combinacoes.len() < TETO_DE_COMBINACOES;

        let mut desvio = None;
        if maduro {
            if !conhecida && cabe {
                // Acusa uma vez: a combinacao entra logo abaixo.
                desvio = Some(Desvio::Combinacao);
            } else if p.ultimo_aviso_por_hora != hora_de_relogio {
                if !conhecida {
                    // O perfil cheio: a combinacao nao entra e seria acusada
                    // a cada pedido. Teto por hora de relogio.
                    desvio = Some(Desvio::Combinacao);
                } else if p.horas[hora] * 100 < p.n * MASSA_RARA_PORCENTO {
                    desvio = Some(Desvio::Hora);
                }
                if desvio.is_some() {
                    p.ultimo_aviso_por_hora = hora_de_relogio;
                }
            }
        }

        p.n = p.n.saturating_add(1);
        p.horas[hora] = p.horas[hora].saturating_add(1);
        if cabe {
            let c = p.combinacoes.entry(k).or_insert(0);
            *c = c.saturating_add(1);
        } else {
            p.coringa = p.coringa.saturating_add(1);
        }
        desvio
    }

    /// Reescreve o arquivo se ha o que gravar e o passo ja andou.
    pub fn gravar_se_passou(&mut self, agora_ms: i64) -> Result<()> {
        match self.a_gravar(agora_ms) {
            Some((caminho, corpo)) => gravar_o_arquivo(&caminho, &corpo),
            None => Ok(()),
        }
    }

    /// O que gravar, quando ha e o passo ja andou: o caminho e o corpo,
    /// para quem chama escrever FORA do `Mutex` desta memoria -- o `fsync`
    /// da troca duravel nao pode fazer o pedido seguinte esperar por ele. A
    /// marca do passo anda aqui, sob o `Mutex`, e e ela que impede duas
    /// escritas ao mesmo tempo no mesmo `.novo`.
    pub fn a_gravar(&mut self, agora_ms: i64) -> Option<(PathBuf, String)> {
        if !self.sujo || agora_ms - self.gravado_ms < PASSO_DE_GRAVAR_MS {
            return None;
        }
        self.gravado_ms = agora_ms;
        self.sujo = false;
        let caminho = self.caminho.clone()?;
        let mut usuarios: Vec<(&String, &Perfil)> = self.mapa.iter().collect();
        usuarios.sort_by(|a, b| a.0.cmp(b.0));
        let mut corpo = String::new();
        for (u, p) in usuarios {
            corpo.push_str(&para_json(u, p).escrever());
            corpo.push('\n');
        }
        Some((caminho, corpo))
    }

    /// Quantos perfis a memoria guarda.
    pub fn quantos(&self) -> usize {
        self.mapa.len()
    }

    /// Quantas combinacoes o perfil do usuario guarda.
    pub fn combinacoes_de(&self, usuario: &str) -> usize {
        self.mapa.get(usuario).map_or(0, |p| p.combinacoes.len())
    }

    /// Pedidos que nao couberam: do usuario que nao coube, e da combinacao
    /// que nao coube no perfil dele.
    pub fn coringa(&self) -> u64 {
        self.coringa_de_usuarios + self.mapa.values().map(|p| p.coringa).sum::<u64>()
    }

    /// O retrato para a op `perfis` (765, P15): o que a memoria guarda, e so
    /// isso -- login, nomes de database e tabela, contagens e horas. Nao ha
    /// valor de linha nem texto de SQL aqui dentro para vazar, e o retrato
    /// tambem nao inventa um: ele sai do MESMO `para_json` que escreve o
    /// arquivo, para as duas formas nunca divergirem.
    ///
    /// `usuario` vazio devolve todos, em ordem de login. O `maduro` diz se o
    /// perfil ja acusa (os dois pisos), que e a pergunta de quem olha a tela:
    /// «este aviso ja vale, ou ainda esta aprendendo?».
    pub fn retrato(&self, usuario: &str, agora_ms: i64) -> Json {
        let mut usuarios: Vec<(&String, &Perfil)> = self
            .mapa
            .iter()
            .filter(|(u, _)| usuario.is_empty() || u.as_str() == usuario)
            .collect();
        usuarios.sort_by(|a, b| a.0.cmp(b.0));
        let lista = usuarios
            .into_iter()
            .map(|(u, p)| {
                let maduro =
                    p.n >= PISO_DE_PEDIDOS && agora_ms - p.primeiro_ms >= PISO_DE_HISTORIA_MS;
                let mut j = para_json(u, p);
                if let Json::Objeto(pares) = &mut j {
                    pares.push(("maduro".to_string(), Json::Bool(maduro)));
                }
                j
            })
            .collect();
        Json::objeto(vec![
            ("perfis", Json::Lista(lista)),
            (
                "coringa_de_usuarios",
                Json::de_u64(self.coringa_de_usuarios),
            ),
            ("piso_de_pedidos", Json::de_u64(PISO_DE_PEDIDOS)),
            (
                "piso_de_historia_dias",
                Json::de_u64((PISO_DE_HISTORIA_MS / (24 * HORA_MS)) as u64),
            ),
            ("massa_rara_porcento", Json::de_u64(MASSA_RARA_PORCENTO)),
            ("teto_de_usuarios", Json::de_u64(TETO_DE_USUARIOS as u64)),
            (
                "teto_de_combinacoes",
                Json::de_u64(TETO_DE_COMBINACOES as u64),
            ),
        ])
    }
}

/// O arquivo inteiro, pela troca duravel: uma queda no meio deixa o de
/// antes.
pub fn gravar_o_arquivo(caminho: &Path, corpo: &str) -> Result<()> {
    if let Some(dir) = caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
        phxsql_store::permissao::criar_diretorio_do_banco(dir)?;
    }
    phxsql_store::sincronia::gravar_duravel(caminho, corpo.as_bytes())
}

fn para_json(u: &str, p: &Perfil) -> Json {
    let mut combinacoes: Vec<(&String, &u64)> = p.combinacoes.iter().collect();
    combinacoes.sort();
    Json::objeto(vec![
        ("usuario", Json::texto_de(u)),
        ("primeiro_ms", Json::de_i64(p.primeiro_ms)),
        ("n", Json::de_u64(p.n)),
        (
            "horas",
            Json::Lista(p.horas.iter().map(|h| Json::de_u64(*h)).collect()),
        ),
        (
            "combinacoes",
            Json::Lista(
                combinacoes
                    .into_iter()
                    .map(|(k, n)| {
                        let mut partes = k.splitn(3, '\u{1f}');
                        Json::Lista(vec![
                            Json::texto_de(partes.next().unwrap_or("")),
                            Json::texto_de(partes.next().unwrap_or("")),
                            Json::texto_de(partes.next().unwrap_or("")),
                            Json::de_u64(*n),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("coringa", Json::de_u64(p.coringa)),
    ])
}

fn de_json(j: &Json) -> Option<(String, Perfil)> {
    let usuario = j.campo("usuario")?.texto()?.to_string();
    if usuario.is_empty() {
        return None;
    }
    let como_u64 = |v: &Json| v.inteiro().filter(|n| *n >= 0).map(|n| n as u64);
    let mut p = Perfil {
        primeiro_ms: j.campo("primeiro_ms")?.inteiro()?,
        n: j.campo("n").and_then(como_u64).unwrap_or(0),
        coringa: j.campo("coringa").and_then(como_u64).unwrap_or(0),
        ultimo_aviso_por_hora: i64::MIN,
        ..Perfil::default()
    };
    if let Some(h) = j.campo("horas").and_then(Json::lista) {
        for (i, v) in h.iter().take(24).enumerate() {
            p.horas[i] = como_u64(v).unwrap_or(0);
        }
    }
    for c in j
        .campo("combinacoes")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .take(TETO_DE_COMBINACOES)
    {
        let Some(c) = c.lista() else { continue };
        let (Some(cat), Some(d), Some(t), Some(n)) = (
            c.first().and_then(Json::texto),
            c.get(1).and_then(Json::texto),
            c.get(2).and_then(Json::texto),
            c.get(3).and_then(como_u64),
        ) else {
            continue;
        };
        p.combinacoes.insert(chave(cat, d, t), n);
    }
    Some((usuario, p))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    /// Meia-noite UTC de um dia qualquer.
    const T0: i64 = 1_800_000_000_000 - 1_800_000_000_000 % (24 * HORA_MS);
    const DIA: i64 = 24 * HORA_MS;

    /// `n` pedidos espalhados pelas 24 horas, para a hora nunca ser rara.
    fn historia(m: &mut Perfis, usuario: &str, cat: &str, tabela: &str, n: u64, desde: i64) {
        for i in 0..n {
            let quando = desde + (i as i64 % 24) * HORA_MS;
            m.registrar(usuario, cat, "b", tabela, quando);
        }
    }

    /// **O aceite da P7:** 300 leituras em `clientes` ha mais de 7 dias, e
    /// depois `excluir` em `folha` -> `Combinacao`; a mesma leitura -> nada;
    /// e o `excluir` em `folha` de novo -> nada (entrou no perfil). RED:
    /// `maduro` sempre falso (sem o perfil acusar) derruba o primeiro
    /// `assert`; sem o `contains_key` (toda combinacao nova) derruba o
    /// segundo.
    #[test]
    fn a_combinacao_nunca_vista_acusa_e_a_habitual_nao() {
        let mut m = Perfis::default();
        historia(&mut m, "ana", categoria("ler"), "clientes", 300, T0);
        let depois = T0 + 8 * DIA + 10 * HORA_MS;
        assert_eq!(
            m.registrar("ana", categoria("excluir"), "b", "folha", depois),
            Some(Desvio::Combinacao)
        );
        assert_eq!(
            m.registrar("ana", categoria("ler"), "b", "clientes", depois + 1),
            None
        );
        assert_eq!(
            m.registrar("ana", categoria("excluir"), "b", "folha", depois + 2),
            None,
            "a combinacao acusada entra no perfil e nao acusa de novo"
        );
    }

    /// Antes dos 200 pedidos, ou antes dos 7 dias, nada acusa. RED: tirar o
    /// `p.n >= PISO_DE_PEDIDOS` (ou o piso de tempo) derruba.
    #[test]
    fn o_usuario_sem_historia_nao_acusa() {
        let mut m = Perfis::default();
        historia(&mut m, "ana", "ler", "clientes", 199, T0);
        let depois = T0 + 30 * DIA;
        assert_eq!(m.registrar("ana", "escrever", "b", "folha", depois), None);
        // 300 pedidos, mas so 2 dias de historia.
        let mut m = Perfis::default();
        historia(&mut m, "bia", "ler", "clientes", 300, T0);
        assert_eq!(
            m.registrar("bia", "escrever", "b", "folha", T0 + 2 * DIA),
            None
        );
    }

    /// A hora rara acusa, e uma vez por hora de relogio. RED: sem o
    /// `ultimo_aviso_por_hora` o segundo pedido da madrugada acusa de novo.
    #[test]
    fn a_hora_rara_acusa_uma_vez_por_hora() {
        let mut m = Perfis::default();
        // 300 pedidos, todos entre 9h e 17h UTC.
        for i in 0..300i64 {
            let dia = i / 30;
            let hora = 9 + (i % 9);
            m.registrar(
                "ana",
                "ler",
                "b",
                "clientes",
                T0 + dia * DIA + hora * HORA_MS,
            );
        }
        let madrugada = T0 + 20 * DIA + 3 * HORA_MS;
        assert_eq!(
            m.registrar("ana", "ler", "b", "clientes", madrugada),
            Some(Desvio::Hora)
        );
        assert_eq!(
            m.registrar("ana", "ler", "b", "clientes", madrugada + 60_000),
            None
        );
        // Na hora habitual, nada.
        assert_eq!(
            m.registrar("ana", "ler", "b", "clientes", T0 + 20 * DIA + 10 * HORA_MS),
            None
        );
    }

    /// O teto: a combinacao 257 nao entra, conta no coringa e acusa uma vez
    /// por hora; o usuario 1.001 nao entra. RED: tirar o `cabe` faz o perfil
    /// crescer.
    #[test]
    fn com_o_teto_cheio_o_perfil_nao_cresce() {
        let mut m = Perfis::default();
        for i in 0..TETO_DE_COMBINACOES + 50 {
            m.registrar("ana", "ler", "b", &format!("t{i}"), T0);
        }
        assert_eq!(m.combinacoes_de("ana"), TETO_DE_COMBINACOES);
        assert_eq!(m.coringa(), 50);
        historia(&mut m, "ana", "ler", "t0", 300, T0);
        let depois = T0 + 8 * DIA;
        assert_eq!(
            m.registrar("ana", "ler", "b", "nova-1", depois),
            Some(Desvio::Combinacao)
        );
        assert_eq!(m.registrar("ana", "ler", "b", "nova-2", depois + 1), None);
        assert_eq!(m.combinacoes_de("ana"), TETO_DE_COMBINACOES);

        let mut m = Perfis::default();
        for i in 0..TETO_DE_USUARIOS + 3 {
            m.registrar(&format!("u{i}"), "ler", "b", "t", T0);
        }
        assert_eq!(m.quantos(), TETO_DE_USUARIOS);
        assert_eq!(m.coringa(), 3);
    }

    /// O nome enorme entra cortado.
    #[test]
    fn o_nome_enorme_entra_cortado() {
        let mut m = Perfis::default();
        let longo = "x".repeat(10_000);
        m.registrar("ana", "ler", &longo, &longo, T0);
        let p = &m.mapa["ana"];
        let k = p.combinacoes.keys().next().unwrap();
        assert!(k.len() <= 3 * TETO_DO_NOME, "{}", k.len());
    }

    /// Persistido: o perfil volta do arquivo depois de reabrir, e acusa como
    /// acusaria sem reiniciar; e o arquivo tem uma linha por usuario.
    /// RED: `gravar_o_arquivo` sem a troca duravel (o `Ok(())` no lugar dela) deixa o
    /// perfil reaberto vazio, e a combinacao nova deixa de acusar.
    #[test]
    fn o_perfil_sobrevive_ao_reinicio() {
        let d = DirTemp::novo("perfis-reinicio");
        let caminho = d.join(NOME_DO_ARQUIVO);
        let mut m = Perfis::abrir(&caminho, T0).unwrap();
        historia(&mut m, "ana", "ler", "clientes", 300, T0);
        // O passo ainda nao andou: nada gravado.
        m.gravar_se_passou(T0 + 1).unwrap();
        assert!(!caminho.exists());
        m.gravar_se_passou(T0 + PASSO_DE_GRAVAR_MS).unwrap();
        let texto = std::fs::read_to_string(&caminho).unwrap();
        assert_eq!(texto.lines().count(), 1, "{texto}");

        let mut m = Perfis::abrir(&caminho, T0).unwrap();
        assert_eq!(m.quantos(), 1);
        let depois = T0 + 8 * DIA;
        assert_eq!(m.registrar("ana", "ler", "b", "clientes", depois), None);
        assert_eq!(
            m.registrar("ana", "escrever", "b", "folha", depois + 1),
            Some(Desvio::Combinacao)
        );
    }

    /// A linha torta nao derruba a memoria.
    #[test]
    fn a_linha_torta_se_ignora() {
        let d = DirTemp::novo("perfis-torta");
        let caminho = d.join(NOME_DO_ARQUIVO);
        std::fs::write(
            &caminho,
            "{\"usuario\":\"ana\",\"primeiro_ms\":1,\"n\":3}\n{torta\n{\"usuario\":\"\"}\n",
        )
        .unwrap();
        let m = Perfis::abrir(&caminho, T0).unwrap();
        assert_eq!(m.quantos(), 1);
    }

    /// A categoria sai do motor da permissao: o `excluir` e o `inserir`
    /// escrevem, o `ler` le, o `ping` e outros.
    #[test]
    fn a_categoria_e_a_da_permissao() {
        assert_eq!(categoria("ler"), "ler");
        assert_eq!(categoria("varrer"), "ler");
        assert_eq!(categoria("excluir"), "escrever");
        assert_eq!(categoria("inserir"), "escrever");
        assert_eq!(categoria("criar_tabela"), "ddl");
        assert_eq!(categoria("excluir_tabela"), "administrar");
        assert_eq!(categoria("ping"), "outros");
    }
}
