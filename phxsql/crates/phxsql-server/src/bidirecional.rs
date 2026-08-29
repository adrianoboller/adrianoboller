//! A parte funda da replicacao bidirecional (multi-master), sem rede.
//!
//! Dois servidores, cada um replica do outro, os dois recebendo escrita. Os
//! dois problemas reais moram aqui, e cada um tem a sua peca:
//!
//! # O laco infinito, e a ORIGEM no evento
//!
//! A alteracao que A aplicou vinda de B nao pode voltar para B. Cada evento do
//! `.log` carrega a origem da escrita ([`hash_id`] do `id_servidor` de onde
//! ela nasceu; zero = local), e o `replicar` com o campo `para` NAO devolve os
//! eventos cuja origem e o proprio destino. A replica ainda descarta por conta
//! propria o que tiver a origem dela -- cinto e suspensorio, porque um source
//! que ignorasse o `para` reabriria o laco.
//!
//! # O conflito, e o carimbo
//!
//! O mesmo registro alterado dos dois lados antes de sincronizar: vence a
//! modificacao MAIS RECENTE, pelo carimbo que o `.log` ja tem por evento.
//! Isso exige uma honestidade dupla:
//!
//! - o evento aplicado guarda o carimbo do NASCIMENTO da escrita, nao o da
//!   chegada (ver `Table::forcar_proximo_evento`) -- senao venceria sempre
//!   quem sincronizou por ultimo;
//! - a regra so e justa com os RELOGIOS SINCRONIZADOS entre os servidores
//!   (NTP). Sem isso, o lado com o relogio adiantado vence sempre. Esta
//!   escrito em `docs/REPLICACAO.md` com todas as letras.
//!
//! Empate de carimbo desempata pela origem numerica MAIOR ([`remoto_vence`]):
//! arbitrario, deterministico, e igual nos dois lados -- que e o que importa
//! para os dois convergirem.
//!
//! # A identidade e a CHAVE, nunca o rowid
//!
//! A ordem de digitacao e sagrada EM CADA SERVIDOR: cada `.reg` mantem a SUA
//! ordem de chegada, e o insert local de A e o de B podem ganhar o mesmo
//! rowid. Entre servidores, a linha se identifica pela chave unica
//! ([`chave_unica`]) -- o mesmo desenho da sincronia do DbLink. Consequencia
//! honesta: **o modo bidirecional exige tabela com chave unica de uma
//! coluna**; sem ela a tabela e recusada com o motivo escrito (o HFSQL(R)
//! tambem impoe identificador adequado para replicar).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use phxsql_core::error::Result;
use phxsql_core::json::Json;
use phxsql_core::schema::Schema;

/// A identidade numerica de um servidor, derivada do `id_servidor`.
///
/// # Por que um numero, e por que 16 bits
///
/// O evento do `.log` tem exatamente 2 bytes reservados para a origem, e o
/// texto nao caberia nem deveria: o cabecalho e de largura fixa. O CRC-32 do
/// texto, dobrado ao meio, da um numero estavel em qualquer maquina.
///
/// Zero fica reservado para "escrita local" -- e todo evento gravado antes de
/// a origem existir le zero, que e a leitura certa para eles.
///
/// # A colisao, dita com honestidade
///
/// Dois ids diferentes PODEM cair no mesmo numero (1 chance em 65.535 por
/// par). Colisao aqui suprimiria eventos de um terceiro servidor inocente.
/// Por isso a replica confere ao conectar: se o hash do id do source bater
/// com o do proprio id sendo os textos diferentes, a rodada para com erro --
/// troca-se um id e acabou. Ver `rodada_bidirecional`.
pub fn hash_id(id: &str) -> u16 {
    let c = phxsql_core::crc::crc32(id.trim().as_bytes());
    let dobrado = ((c >> 16) ^ (c & 0xFFFF)) as u16;
    if dobrado == 0 {
        1
    } else {
        dobrado
    }
}

/// O ultimo toque conhecido numa chave: quando, por quem, e se foi exclusao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toque {
    pub carimbo: i64,
    /// [`hash_id`] de quem escreveu. Para escrita local, o hash do PROPRIO
    /// servidor -- e nao zero --, para o empate desempatar igual nos dois
    /// lados.
    pub origem: u16,
    pub excluido: bool,
}

/// O evento remoto vence o ultimo toque local?
///
/// Mais recente vence; empate de carimbo vai para a origem numerica maior.
/// Igual dos dois lados por construcao: os dois comparam os mesmos numeros.
/// Empate total (mesmo carimbo, mesma origem) NAO vence -- e o que faz a
/// reaplicacao de um evento ja visto ser inofensiva.
pub fn remoto_vence(carimbo: i64, origem: u16, local: &Toque) -> bool {
    carimbo > local.carimbo || (carimbo == local.carimbo && origem > local.origem)
}

/// A chave unica de UMA coluna que identifica a linha entre servidores.
///
/// Na ordem: a chave primaria, senao o primeiro indice unico de uma coluna.
/// `None` = a tabela nao tem identidade replicavel, e o modo bidirecional a
/// recusa com o motivo escrito. Chave COMPOSTA tambem fica de fora por
/// enquanto -- mesma regra da sincronia do DbLink, ate alguem precisar dela
/// com o pedido na mesa.
pub fn chave_unica(esquema: &Schema) -> Option<(String, usize)> {
    let serve = |i: &phxsql_core::schema::IndexDef| {
        i.unico && i.colunas.len() == 1 && {
            let nome = &esquema.colunas()[i.colunas[0].coluna].nome;
            nome != "softdeleted" && nome != "rownum"
        }
    };
    esquema
        .indices()
        .iter()
        .find(|i| i.primario && serve(i))
        .or_else(|| esquema.indices().iter().find(|i| serve(i)))
        .map(|i| (i.nome.clone(), i.colunas[0].coluna))
}

/// O que a replica bidirecional lembra de cada tabela, em memoria.
///
/// Reconstruido do proprio `.log` local: `vistos` diz ate onde a varredura
/// chegou, e o mapa guarda o ultimo toque por chave. Perder isto custa uma
/// varredura, nunca um dado -- mesma filosofia da marca do diario.
#[derive(Debug, Default)]
pub struct MapaDeToques {
    /// Eventos do diario local ja absorvidos no mapa.
    pub vistos: u64,
    /// Chave canonica -> ultimo toque.
    pub toques: HashMap<String, Toque>,
}

/// O que o laco de uma origem conta para a operacao `replicacao_estado`.
#[derive(Debug, Default, Clone)]
pub struct EstadoOrigem {
    /// "streaming", "cada_15min" ou "diaria_02:30".
    pub modo: String,
    pub ultima_rodada_ms: i64,
    /// Eventos aplicados desde o arranque, somando todas as tabelas.
    pub aplicados: u64,
    pub ultimo_erro: String,
    /// Tabelas recusadas e o motivo -- ex.: sem chave unica no modo multi.
    pub recusas: BTreeMap<String, String>,
    /// "database/tabela" -> posicao consumida na origem.
    pub posicoes: BTreeMap<String, u64>,
    /// Proxima janela do agendamento, ms desde a epoca. Zero = streaming.
    pub proxima_janela_ms: i64,
}

impl EstadoOrigem {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("modo", Json::texto_de(&self.modo)),
            (
                "ultima_rodada",
                if self.ultima_rodada_ms == 0 {
                    Json::Nulo
                } else {
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.ultima_rodada_ms))
                },
            ),
            ("aplicados", Json::de_u64(self.aplicados)),
            (
                "ultimo_erro",
                if self.ultimo_erro.is_empty() {
                    Json::Nulo
                } else {
                    Json::texto_de(&self.ultimo_erro)
                },
            ),
            (
                "recusas",
                Json::Objeto(
                    self.recusas
                        .iter()
                        .map(|(k, v)| (k.clone(), Json::texto_de(v)))
                        .collect(),
                ),
            ),
            (
                "posicoes",
                Json::Objeto(
                    self.posicoes
                        .iter()
                        .map(|(k, v)| (k.clone(), Json::de_u64(*v)))
                        .collect(),
                ),
            ),
            (
                "proxima_janela",
                if self.proxima_janela_ms == 0 {
                    Json::Nulo
                } else {
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.proxima_janela_ms))
                },
            ),
        ])
    }
}

/// Quantos milissegundos dormir ate a proxima janela do agendamento.
///
/// `cada_minutos` e um intervalo simples a partir de agora; `hora` e a
/// proxima ocorrencia daquele minuto do dia, em UTC -- a MESMA convencao do
/// backup agendado, para as duas agendas do servidor nao discordarem de fuso.
/// Com os dois vazios devolve zero, que e o streaming.
pub fn ms_ate_a_janela(agora_ms: i64, cada_minutos: u64, hora: &str) -> i64 {
    if cada_minutos > 0 {
        return cada_minutos as i64 * 60_000;
    }
    let Some(alvo_min) = crate::config::Backup::minuto_do_dia(hora) else {
        return 0;
    };
    let no_dia = agora_ms.rem_euclid(86_400_000);
    let alvo = alvo_min as i64 * 60_000;
    if alvo > no_dia {
        alvo - no_dia
    } else {
        alvo - no_dia + 86_400_000
    }
}

/// As posicoes consumidas por origem, num arquivo ao lado dos dados.
///
/// # Por que um arquivo, e por que perder ele nao e grave
///
/// No modo A a posicao e o proprio diario da replica: cada evento aplicado
/// gera exatamente um evento local. No bidirecional isso quebra -- o diario
/// local mistura escrita local com aplicada, e eventos suprimidos pelo `para`
/// avancam a posicao sem gerar nada aqui. Entao a posicao consumida vira
/// estado proprio, gravado aqui.
///
/// Perder o arquivo recomeca do zero, e recomecar e INOFENSIVO: a aplicacao e
/// por chave com "mais recente vence", e reaplicar um evento ja visto perde
/// para o toque igual que ja esta no mapa. Custa releitura, nunca dado.
pub fn ler_posicoes(caminho: &Path) -> HashMap<String, u64> {
    let Ok(texto) = std::fs::read_to_string(caminho) else {
        return HashMap::new();
    };
    let Ok(j) = Json::analisar(&texto) else {
        return HashMap::new();
    };
    match j {
        Json::Objeto(pares) => pares
            .into_iter()
            .filter_map(|(k, v)| v.numero().map(|n| (k, n.max(0.0) as u64)))
            .collect(),
        _ => HashMap::new(),
    }
}

pub fn gravar_posicoes(caminho: &Path, posicoes: &HashMap<String, u64>) -> Result<()> {
    let mut pares: Vec<(String, Json)> = posicoes
        .iter()
        .map(|(k, v)| (k.clone(), Json::de_u64(*v)))
        .collect();
    pares.sort_by(|a, b| a.0.cmp(&b.0));
    std::fs::write(caminho, Json::Objeto(pares).escrever())?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    // ------------------------------------------------------------- o hash

    #[test]
    fn hash_e_estavel_nunca_zero_e_distingue_ids_normais() {
        assert_eq!(hash_id("curitiba-01"), hash_id("curitiba-01"));
        assert_eq!(hash_id(" curitiba-01 "), hash_id("curitiba-01"));
        assert_ne!(hash_id("curitiba-01"), hash_id("belgica-01"));
        assert_ne!(hash_id("a"), 0);
        assert_ne!(hash_id(""), 0, "zero e reservado para escrita local");
    }

    // --------------------------------------------------------- o conflito

    #[test]
    fn mais_recente_vence_nos_dois_sentidos() {
        let local = Toque {
            carimbo: 1_000,
            origem: 10,
            excluido: false,
        };
        assert!(remoto_vence(1_001, 5, &local), "remoto mais novo vence");
        assert!(!remoto_vence(999, 5, &local), "remoto mais velho perde");
    }

    /// O empate desempata pela origem MAIOR -- e o desenho garante que os dois
    /// servidores fazem a MESMA conta, entao exatamente um lado aplica e os
    /// dois convergem.
    #[test]
    fn empate_de_carimbo_desempata_pela_origem_e_e_simetrico() {
        let (a, b) = (hash_id("servidor-a"), hash_id("servidor-b"));
        assert_ne!(a, b, "o teste precisa de hashes distintos");

        // No servidor A: toque local com origem a; chega o evento de B.
        let em_a = Toque {
            carimbo: 500,
            origem: a,
            excluido: false,
        };
        // No servidor B: toque local com origem b; chega o evento de A.
        let em_b = Toque {
            carimbo: 500,
            origem: b,
            excluido: false,
        };
        let b_vence_em_a = remoto_vence(500, b, &em_a);
        let a_vence_em_b = remoto_vence(500, a, &em_b);
        assert_ne!(
            b_vence_em_a, a_vence_em_b,
            "exatamente UM lado aplica; os dois aplicando desfariam um ao outro"
        );
    }

    /// Reaplicar o proprio evento (posicao recomecada do zero) nao vence: o
    /// toque igual ja esta no mapa, e igual nao e maior.
    #[test]
    fn reaplicacao_do_mesmo_evento_e_inofensiva() {
        let toque = Toque {
            carimbo: 500,
            origem: 7,
            excluido: false,
        };
        assert!(!remoto_vence(500, 7, &toque));
    }

    // ------------------------------------------------------------ a chave

    fn esquema(indices: Vec<IndexDef>) -> Schema {
        Schema::new(
            "t",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("cpf", ColumnType::Str(11)),
                Column::new("nome", ColumnType::Str(40)),
            ],
            indices,
        )
        .unwrap()
    }

    #[test]
    fn chave_unica_prefere_a_primaria() {
        let e = esquema(vec![
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico(),
            IndexDef::new("porId", vec![IndexColumn::asc(0)])
                .unico()
                .primaria(),
        ]);
        let (indice, pos) = chave_unica(&e).unwrap();
        assert_eq!(indice, "porId");
        assert_eq!(pos, 0);
    }

    #[test]
    fn sem_primaria_serve_o_primeiro_unico_de_uma_coluna() {
        let e = esquema(vec![
            IndexDef::new("porNome", vec![IndexColumn::asc(2)]),
            IndexDef::new("porCpf", vec![IndexColumn::asc(1)]).unico(),
        ]);
        let (indice, pos) = chave_unica(&e).unwrap();
        assert_eq!(indice, "porCpf");
        assert_eq!(pos, 1);
    }

    /// Sem chave unica nao ha identidade entre servidores: a tabela e recusada
    /// no modo bidirecional, com o motivo escrito -- e composta idem.
    #[test]
    fn sem_chave_unica_ou_com_composta_nao_ha_identidade() {
        let sem = esquema(vec![IndexDef::new("porNome", vec![IndexColumn::asc(2)])]);
        assert!(chave_unica(&sem).is_none());

        let composta = esquema(vec![IndexDef::new(
            "porIdCpf",
            vec![IndexColumn::asc(0), IndexColumn::asc(1)],
        )
        .unico()]);
        assert!(chave_unica(&composta).is_none());
    }

    // -------------------------------------------------------- o agendador

    #[test]
    fn cada_minutos_e_um_intervalo_simples() {
        assert_eq!(ms_ate_a_janela(123, 15, ""), 15 * 60_000);
        assert_eq!(ms_ate_a_janela(123, 1, "ignorada"), 60_000);
    }

    #[test]
    fn hora_marcada_e_a_proxima_ocorrencia_no_dia() {
        let meia_noite = 20_000i64 * 86_400_000;
        // 01:00 pedindo 02:30 -> 1h30 de espera.
        assert_eq!(
            ms_ate_a_janela(meia_noite + 3_600_000, 0, "02:30"),
            5_400_000
        );
        // 03:00 pedindo 02:30 -> amanha.
        assert_eq!(
            ms_ate_a_janela(meia_noite + 3 * 3_600_000, 0, "02:30"),
            86_400_000 - 1_800_000
        );
        // Sem agenda nenhuma: zero, que e streaming.
        assert_eq!(ms_ate_a_janela(meia_noite, 0, ""), 0);
    }

    // -------------------------------------------------------- as posicoes

    #[test]
    fn posicoes_atravessam_o_arquivo_e_arquivo_sumido_recomeca_do_zero() {
        let dir = std::env::temp_dir().join(format!("phx-posicoes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let caminho = dir.join("replicacao-posicoes.json");

        assert!(ler_posicoes(&caminho).is_empty(), "sem arquivo, do zero");

        let mut p = HashMap::new();
        p.insert("b|loja/clientes".to_string(), 1234u64);
        p.insert("b|loja/pedidos".to_string(), 7u64);
        gravar_posicoes(&caminho, &p).unwrap();
        assert_eq!(ler_posicoes(&caminho), p);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
