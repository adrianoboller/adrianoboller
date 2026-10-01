//! O registro de rotinas: gatilhos e procedimentos, por database.
//!
//! # Onde mora, e por que em JSON
//!
//! Cada database guarda os seus em dois arquivos no proprio diretorio —
//! `gatilhos.json` e `procedimentos.json`. JSON e nao formato binario porque
//! isto e CADASTRO, nao dado: muda por comando, se le no olho, e viaja junto
//! com o backup do diretorio. Arquivo ausente = zero rotinas = comportamento
//! de sempre; e quando a ultima rotina de um database sai, o arquivo sai
//! junto, para o ausente continuar significando o que significa.
//!
//! # O compilado vive na memoria; o texto vive no disco
//!
//! O disco guarda o corpo como o autor escreveu (e o `SHOW` o devolve
//! verbatim). Ao carregar, cada corpo e compilado UMA vez; o disparo usa o
//! compilado e nunca analisa texto — a licao do Profiler, aplicada antes de
//! doer. Corpo que nao compila mais (um arquivo editado a mao, uma versao
//! velha) NAO e pulado em silencio: ele fica marcado quebrado e **barra a
//! escrita da tabela dele** com o motivo — pular seria fingir que a regra que
//! o dono escreveu nao existe, no exato momento em que ela deixou de valer.

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_sql::rotina::{
    analisar_corpo, regras_de_gatilho, regras_de_procedimento, tipo_de_texto, Evento, GatilhoDef,
    Instrucao, Modo, Parametro, ProcedimentoDef, Quando,
};

pub const ARQUIVO_GATILHOS: &str = "gatilhos.json";
pub const ARQUIVO_PROCEDIMENTOS: &str = "procedimentos.json";

/// Qual dos dois arquivos de um database uma mudanca deixou devendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arquivo {
    Gatilhos,
    Procedimentos,
}

/// O que uma mudanca no registro deixou devendo ao disco -- pedido 595.
///
/// As mudancas mexem so na MEMORIA, sob a trava do registro, e devolvem isto;
/// o disco vem depois, por [`Rotinas::retrato`] e [`Retrato::gravar`]. A
/// razao e a trava: o registro e lido em toda escrita de tabela com gatilho,
/// e o `excluir_tabela` o toma com a trava GLOBAL na mao. Gravar ali dentro
/// poria dois `fsync` sob a trava global (a catraca `alcancam-fsync-2`) e
/// pararia toda escrita com gatilho enquanto o disco responde.
#[must_use = "a mudanca so vale no disco depois de Retrato::gravar"]
#[derive(Debug)]
pub struct PorGravar {
    db: String,
    arquivo: Arquivo,
}

/// O conteudo que um arquivo de cadastro deve ter AGORA, tirado sob a trava
/// do registro e gravado fora dela. `None` e «nao deve existir»: ausente
/// quer dizer zero rotinas (ou zero visoes), e um `[]` no disco seria um
/// terceiro estado que nada le.
///
/// O retrato e do estado INTEIRO no instante em que se tira, e nao da
/// mudanca que o pediu: quem grava por ultimo grava o mais novo, e uma
/// mudanca alheia que entrou no meio vai junto em vez de ser desfeita. Quem
/// serializa os retratos e o servidor (`cadastro_no_disco`), para dois nao
/// trocarem de ordem entre tirar e gravar.
#[must_use = "o retrato so vale depois de gravar"]
#[derive(Debug)]
pub struct Retrato {
    arquivo: PathBuf,
    corpo: Option<String>,
}

impl Retrato {
    pub(crate) fn novo(arquivo: PathBuf, corpo: Option<String>) -> Retrato {
        Retrato { arquivo, corpo }
    }

    /// Leva o retrato ao disco, de forma duravel -- pedido 595.
    ///
    /// Regravar e pelo motor da casa (`sincronia::gravar_duravel`):
    /// temporario, `fsync`, troca e `fsync` da pasta. Era `escrever_do_banco`
    /// NO LUGAR e sem `fsync` nenhum: uma queda no meio deixava o JSON pela
    /// metade, e JSON invalido derruba o arranque. Apagar e o `unlink` mais o
    /// `fsync` da pasta, pelo mesmo `PorSincronizar` do `excluir_tabela`
    /// (591): sem ele, o gatilho excluido volta numa queda.
    pub fn gravar(self) -> Result<()> {
        match self.corpo {
            Some(corpo) => phxsql_store::sincronia::gravar_duravel(&self.arquivo, corpo.as_bytes()),
            None => {
                match std::fs::remove_file(&self.arquivo) {
                    Ok(()) => {}
                    // Ja ausente e o estado pedido; um retrato anterior o
                    // apagou e ja levou a pasta ao disco.
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                    Err(e) => return Err(e.into()),
                }
                phxsql_store::catalogo::PorSincronizar::entradas_que_sairam(vec![self.arquivo])
                    .levar_ao_disco()?;
                Ok(())
            }
        }
    }
}

/// Os gatilhos de uma escrita, ja separados: os que rodam ANTES e os DEPOIS.
///
/// Tem nome proprio porque o par viaja junto por todo o caminho de escrita e
/// os dois lados nao sao intercambiaveis -- o primeiro roda com a trava na
/// mao e pode cancelar; o segundo roda solto e so avisa.
pub type AntesEDepois = (Vec<Arc<Gatilho>>, Vec<Arc<Gatilho>>);

/// Um gatilho guardado, com o corpo ja compilado (ou o motivo de nao).
#[derive(Debug)]
pub struct Gatilho {
    pub nome: String,
    pub quando: Quando,
    pub evento: Evento,
    /// Qualificada (`schema.tabela`) quando ha schema — exatamente como o
    /// campo `tabela` do protocolo escreve, porque e contra ele que o
    /// disparo compara.
    pub tabela: String,
    pub corpo: String,
    pub criado_em: String,
    pub criado_por: String,
    pub programa: std::result::Result<Vec<Instrucao>, String>,
}

impl Gatilho {
    pub fn para_json(&self) -> Json {
        let mut pares = vec![
            ("nome", Json::texto_de(&self.nome)),
            ("tabela", Json::texto_de(&self.tabela)),
            ("quando", Json::texto_de(self.quando.nome())),
            ("evento", Json::texto_de(self.evento.nome())),
            ("corpo", Json::texto_de(&self.corpo)),
            ("criado_em", Json::texto_de(&self.criado_em)),
            ("criado_por", Json::texto_de(&self.criado_por)),
        ];
        if let Err(motivo) = &self.programa {
            // Um gatilho quebrado aparece quebrado na listagem — e a unica
            // chance de alguem consertar antes de a escrita esbarrar nele.
            pares.push(("quebrado", Json::texto_de(motivo)));
        }
        Json::objeto(pares)
    }

    fn para_disco(&self) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(&self.nome)),
            ("tabela", Json::texto_de(&self.tabela)),
            ("quando", Json::texto_de(self.quando.nome())),
            ("evento", Json::texto_de(self.evento.nome())),
            ("corpo", Json::texto_de(&self.corpo)),
            ("criado_em", Json::texto_de(&self.criado_em)),
            ("criado_por", Json::texto_de(&self.criado_por)),
        ])
    }

    fn do_disco(j: &Json, arquivo: &Path) -> Result<Gatilho> {
        let nome = j.texto_ou("nome", "").to_string();
        let tabela = j.texto_ou("tabela", "").to_string();
        let quando = Quando::de_texto(j.texto_ou("quando", "")).ok_or_else(|| {
            PhxError::Esquema(format!(
                "{}: gatilho {nome:?} com \"quando\" invalido (BEFORE/AFTER)",
                arquivo.display()
            ))
        })?;
        let evento = Evento::de_texto(j.texto_ou("evento", "")).ok_or_else(|| {
            PhxError::Esquema(format!(
                "{}: gatilho {nome:?} com \"evento\" invalido (INSERT/UPDATE/DELETE)",
                arquivo.display()
            ))
        })?;
        if nome.is_empty() || tabela.is_empty() {
            return Err(PhxError::Esquema(format!(
                "{}: gatilho sem nome ou sem tabela",
                arquivo.display()
            )));
        }
        let corpo = j.texto_ou("corpo", "").to_string();
        let programa =
            analisar_corpo(&corpo, &regras_de_gatilho(quando, evento)).map_err(|e| e.to_string());
        Ok(Gatilho {
            nome,
            quando,
            evento,
            tabela,
            corpo,
            criado_em: j.texto_ou("criado_em", "").to_string(),
            criado_por: j.texto_ou("criado_por", "").to_string(),
            programa,
        })
    }
}

/// Um procedimento guardado, com o corpo ja compilado (ou o motivo de nao).
#[derive(Debug)]
pub struct Procedimento {
    pub nome: String,
    pub parametros: Vec<Parametro>,
    pub corpo: String,
    pub criado_em: String,
    pub criado_por: String,
    pub programa: std::result::Result<Vec<Instrucao>, String>,
}

impl Procedimento {
    fn parametros_json(&self) -> Json {
        Json::Lista(
            self.parametros
                .iter()
                .map(|p| {
                    Json::objeto(vec![
                        ("modo", Json::texto_de(p.modo.nome())),
                        ("nome", Json::texto_de(&p.nome)),
                        ("tipo", Json::texto_de(&p.tipo_escrito)),
                    ])
                })
                .collect(),
        )
    }

    pub fn para_json(&self) -> Json {
        let mut pares = vec![
            ("nome", Json::texto_de(&self.nome)),
            ("parametros", self.parametros_json()),
            ("corpo", Json::texto_de(&self.corpo)),
            ("criado_em", Json::texto_de(&self.criado_em)),
            ("criado_por", Json::texto_de(&self.criado_por)),
        ];
        if let Err(motivo) = &self.programa {
            pares.push(("quebrado", Json::texto_de(motivo)));
        }
        Json::objeto(pares)
    }

    fn para_disco(&self) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(&self.nome)),
            ("parametros", self.parametros_json()),
            ("corpo", Json::texto_de(&self.corpo)),
            ("criado_em", Json::texto_de(&self.criado_em)),
            ("criado_por", Json::texto_de(&self.criado_por)),
        ])
    }

    fn do_disco(j: &Json, arquivo: &Path) -> Result<Procedimento> {
        let nome = j.texto_ou("nome", "").to_string();
        if nome.is_empty() {
            return Err(PhxError::Esquema(format!(
                "{}: procedimento sem nome",
                arquivo.display()
            )));
        }
        let mut parametros = Vec::new();
        for p in j.campo("parametros").and_then(Json::lista).unwrap_or(&[]) {
            let modo = Modo::de_texto(p.texto_ou("modo", "IN")).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "{}: procedimento {nome:?} com modo de parametro invalido",
                    arquivo.display()
                ))
            })?;
            let tipo_escrito = p.texto_ou("tipo", "").to_string();
            let tipo = tipo_de_texto(&tipo_escrito).map_err(|e| {
                PhxError::Esquema(format!("{}: procedimento {nome:?}: {e}", arquivo.display()))
            })?;
            parametros.push(Parametro {
                modo,
                nome: p.texto_ou("nome", "").to_lowercase(),
                tipo,
                tipo_escrito,
            });
        }
        let corpo = j.texto_ou("corpo", "").to_string();
        let programa = analisar_corpo(&corpo, &regras_de_procedimento()).map_err(|e| e.to_string());
        Ok(Procedimento {
            nome,
            parametros,
            corpo,
            criado_em: j.texto_ou("criado_em", "").to_string(),
            criado_por: j.texto_ou("criado_por", "").to_string(),
            programa,
        })
    }
}

#[derive(Debug, Default)]
struct Db {
    gatilhos: Vec<Arc<Gatilho>>,
    procedimentos: Vec<Arc<Procedimento>>,
}

/// O registro inteiro, um `Db` por database que tem alguma rotina.
#[derive(Debug)]
pub struct Rotinas {
    base: PathBuf,
    dbs: HashMap<String, Db>,
}

impl Rotinas {
    /// Carrega tudo que existe embaixo de `base`.
    ///
    /// Um arquivo que nao e JSON valido DERRUBA a subida, com o caminho e o
    /// motivo: subir sem os gatilhos que o dono escreveu seria gravar sem as
    /// regras dele, em silencio — pior que nao subir.
    pub fn carregar(base: &Path) -> Result<Rotinas> {
        let mut dbs = HashMap::new();
        if base.is_dir() {
            for entrada in std::fs::read_dir(base)? {
                let entrada = entrada?;
                if !entrada.path().is_dir() {
                    continue;
                }
                let Some(nome_db) = entrada.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                let db = Rotinas::carregar_db(&entrada.path())?;
                if !db.gatilhos.is_empty() || !db.procedimentos.is_empty() {
                    dbs.insert(nome_db, db);
                }
            }
        }
        Ok(Rotinas {
            base: base.to_path_buf(),
            dbs,
        })
    }

    fn carregar_db(dir: &Path) -> Result<Db> {
        let mut db = Db::default();
        let arq_g = dir.join(ARQUIVO_GATILHOS);
        if arq_g.is_file() {
            let texto = std::fs::read_to_string(&arq_g)?;
            let j = Json::analisar(&texto)
                .map_err(|e| PhxError::Esquema(format!("{}: {e}", arq_g.display())))?;
            for g in j.campo("gatilhos").and_then(Json::lista).unwrap_or(&[]) {
                db.gatilhos.push(Arc::new(Gatilho::do_disco(g, &arq_g)?));
            }
        }
        let arq_p = dir.join(ARQUIVO_PROCEDIMENTOS);
        if arq_p.is_file() {
            let texto = std::fs::read_to_string(&arq_p)?;
            let j = Json::analisar(&texto)
                .map_err(|e| PhxError::Esquema(format!("{}: {e}", arq_p.display())))?;
            for p in j
                .campo("procedimentos")
                .and_then(Json::lista)
                .unwrap_or(&[])
            {
                db.procedimentos
                    .push(Arc::new(Procedimento::do_disco(p, &arq_p)?));
            }
        }
        Ok(db)
    }

    /// Ha algum gatilho em algum database? E o que abastece o portao atomico
    /// do servidor — o caminho sem gatilho nenhum nao chega nem aqui.
    pub fn ha_gatilhos(&self) -> bool {
        self.dbs.values().any(|d| !d.gatilhos.is_empty())
    }

    /// Os gatilhos de uma tabela e evento, ja separados em (BEFORE, AFTER),
    /// na ordem de criacao. Clona `Arc`s para a trava do registro poder ser
    /// solta antes de qualquer corpo rodar.
    pub fn gatilhos_de(&self, db: &str, tabela: &str, evento: Evento) -> AntesEDepois {
        let mut antes = Vec::new();
        let mut depois = Vec::new();
        if let Some(d) = self.dbs.get(db) {
            for g in &d.gatilhos {
                if g.evento == evento && g.tabela == tabela {
                    match g.quando {
                        Quando::Antes => antes.push(Arc::clone(g)),
                        Quando::Depois => depois.push(Arc::clone(g)),
                    }
                }
            }
        }
        (antes, depois)
    }

    pub fn gatilhos_do_db(&self, db: &str) -> Vec<Arc<Gatilho>> {
        self.dbs
            .get(db)
            .map(|d| d.gatilhos.clone())
            .unwrap_or_default()
    }

    pub fn procedimentos_do_db(&self, db: &str) -> Vec<Arc<Procedimento>> {
        self.dbs
            .get(db)
            .map(|d| d.procedimentos.clone())
            .unwrap_or_default()
    }

    pub fn procedimento(&self, db: &str, nome: &str) -> Option<Arc<Procedimento>> {
        self.dbs
            .get(db)?
            .procedimentos
            .iter()
            .find(|p| p.nome.eq_ignore_ascii_case(nome))
            .cloned()
    }

    pub fn criar_gatilho(
        &mut self,
        db: &str,
        def: GatilhoDef,
        criado_por: &str,
    ) -> Result<(Arc<Gatilho>, PorGravar)> {
        self.conferir_nome_do_db(db)?;
        let d = self.dbs.entry(db.to_string()).or_default();
        if d.gatilhos
            .iter()
            .any(|g| g.nome.eq_ignore_ascii_case(&def.nome))
        {
            return Err(PhxError::Duplicado(format!(
                "ja existe um gatilho {:?} em {db}; DROP TRIGGER antes",
                def.nome
            )));
        }
        // O corpo ja foi validado no parse do CREATE; compilar de novo aqui e
        // barato e garante que o que entra no registro NUNCA esta quebrado.
        let programa = analisar_corpo(&def.corpo, &regras_de_gatilho(def.quando, def.evento))
            .map_err(|e| e.to_string());
        let g = Arc::new(Gatilho {
            nome: def.nome,
            quando: def.quando,
            evento: def.evento,
            tabela: def.tabela,
            corpo: def.corpo,
            criado_em: phxsql_core::datahora::instante_iso(crate::agora_ms()),
            criado_por: criado_por.to_string(),
            programa,
        });
        d.gatilhos.push(Arc::clone(&g));
        Ok((g, PorGravar::de(db, Arquivo::Gatilhos)))
    }

    /// Devolve `None` quando o gatilho nao existia -- e nada a gravar.
    pub fn excluir_gatilho(&mut self, db: &str, nome: &str) -> Option<PorGravar> {
        let d = self.dbs.get_mut(db)?;
        let antes = d.gatilhos.len();
        d.gatilhos.retain(|g| !g.nome.eq_ignore_ascii_case(nome));
        (d.gatilhos.len() != antes).then(|| PorGravar::de(db, Arquivo::Gatilhos))
    }

    /// Tira todos os gatilhos de uma tabela — chamada quando a tabela e
    /// excluida, como o MySQL(R) faz: gatilho orfao dispararia contra uma
    /// homonima futura que nao tem nada a ver com ele.
    pub fn excluir_gatilhos_da_tabela(
        &mut self,
        db: &str,
        tabela: &str,
    ) -> (usize, Option<PorGravar>) {
        let Some(d) = self.dbs.get_mut(db) else {
            return (0, None);
        };
        let antes = d.gatilhos.len();
        d.gatilhos.retain(|g| g.tabela != tabela);
        let saiu = antes - d.gatilhos.len();
        (
            saiu,
            (saiu > 0).then(|| PorGravar::de(db, Arquivo::Gatilhos)),
        )
    }

    pub fn criar_procedimento(
        &mut self,
        db: &str,
        def: ProcedimentoDef,
        criado_por: &str,
    ) -> Result<(Arc<Procedimento>, PorGravar)> {
        self.conferir_nome_do_db(db)?;
        let d = self.dbs.entry(db.to_string()).or_default();
        if d.procedimentos
            .iter()
            .any(|p| p.nome.eq_ignore_ascii_case(&def.nome))
        {
            return Err(PhxError::Duplicado(format!(
                "ja existe um procedimento {:?} em {db}; DROP PROCEDURE antes",
                def.nome
            )));
        }
        let programa =
            analisar_corpo(&def.corpo, &regras_de_procedimento()).map_err(|e| e.to_string());
        let p = Arc::new(Procedimento {
            nome: def.nome,
            parametros: def.parametros,
            corpo: def.corpo,
            criado_em: phxsql_core::datahora::instante_iso(crate::agora_ms()),
            criado_por: criado_por.to_string(),
            programa,
        });
        d.procedimentos.push(Arc::clone(&p));
        Ok((p, PorGravar::de(db, Arquivo::Procedimentos)))
    }

    /// Devolve `None` quando o procedimento nao existia -- e nada a gravar.
    pub fn excluir_procedimento(&mut self, db: &str, nome: &str) -> Option<PorGravar> {
        let d = self.dbs.get_mut(db)?;
        let antes = d.procedimentos.len();
        d.procedimentos
            .retain(|p| !p.nome.eq_ignore_ascii_case(nome));
        (d.procedimentos.len() != antes).then(|| PorGravar::de(db, Arquivo::Procedimentos))
    }

    /// O nome do db vira caminho de arquivo logo abaixo; a sonda de
    /// travessia roda aqui TAMBEM (alem do despachar), porque este nome pode
    /// ter vindo de dentro do texto SQL, que a sonda de fora nao ve.
    fn conferir_nome_do_db(&self, db: &str) -> Result<()> {
        if db.is_empty() || phxsql_store::catalogo::nome_hostil(db) {
            return Err(PhxError::Autorizacao(format!(
                "database {db:?} nao e um nome"
            )));
        }
        if !self.base.join(db).is_dir() {
            return Err(PhxError::NaoEncontrado(format!("database {db} nao existe")));
        }
        Ok(())
    }

    /// O que o arquivo devido deve conter AGORA -- ver [`Retrato`]. Barato
    /// de proposito (serializar algumas dezenas de rotinas), porque roda com
    /// a trava do registro na mao; o disco fica para [`Retrato::gravar`].
    pub fn retrato(&self, devido: &PorGravar) -> Retrato {
        let (nome_do_arquivo, chave, lista): (&str, &str, Vec<Json>) = match devido.arquivo {
            Arquivo::Gatilhos => (
                ARQUIVO_GATILHOS,
                "gatilhos",
                self.dbs
                    .get(&devido.db)
                    .map(|d| d.gatilhos.iter().map(|g| g.para_disco()).collect())
                    .unwrap_or_default(),
            ),
            Arquivo::Procedimentos => (
                ARQUIVO_PROCEDIMENTOS,
                "procedimentos",
                self.dbs
                    .get(&devido.db)
                    .map(|d| d.procedimentos.iter().map(|p| p.para_disco()).collect())
                    .unwrap_or_default(),
            ),
        };
        let arquivo = self.base.join(&devido.db).join(nome_do_arquivo);
        // Lista vazia = arquivo ausente; um vazio diria o mesmo com mais um
        // arquivo para alguem estranhar no backup.
        let corpo = (!lista.is_empty())
            .then(|| Json::objeto(vec![(chave, Json::Lista(lista))]).escrever_identado());
        Retrato::novo(arquivo, corpo)
    }

    /// O retrato tirado e gravado na mesma chamada, com o registro na mao.
    /// Para quem nao divide o registro com ninguem (os testes, uma
    /// ferramenta); o servidor tira sob a trava e grava fora dela.
    pub fn gravar(&self, devido: PorGravar) -> Result<()> {
        self.retrato(&devido).gravar()
    }
}

impl PorGravar {
    fn de(db: &str, arquivo: Arquivo) -> PorGravar {
        PorGravar {
            db: db.to_string(),
            arquivo,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn dir_temp(nome: &str) -> DirTemp {
        let d = DirTemp::novo(&format!("rotinas-{nome}"));
        std::fs::create_dir_all(d.join("loja")).unwrap();
        d
    }

    fn gatilho_def(nome: &str, tabela: &str) -> GatilhoDef {
        GatilhoDef {
            nome: nome.into(),
            quando: Quando::Antes,
            evento: Evento::Inserir,
            database: String::new(),
            tabela: tabela.into(),
            corpo: "SET NEW.uf = UPPER(NEW.uf)".into(),
        }
    }

    /// **A persistencia e a volta dela.** Criar, recarregar de outro registro
    /// e encontrar o gatilho compilado e funcionando.
    #[test]
    fn cria_grava_e_recarrega() {
        let base = dir_temp("volta");
        let mut r = Rotinas::carregar(&base).unwrap();
        assert!(!r.ha_gatilhos());
        let (_, devido) = r
            .criar_gatilho("loja", gatilho_def("normaliza", "clientes"), "root")
            .unwrap();
        r.gravar(devido).unwrap();
        assert!(base.join("loja").join(ARQUIVO_GATILHOS).is_file());

        let r2 = Rotinas::carregar(&base).unwrap();
        assert!(r2.ha_gatilhos());
        let (antes, depois) = r2.gatilhos_de("loja", "clientes", Evento::Inserir);
        assert_eq!(antes.len(), 1);
        assert_eq!(depois.len(), 0);
        assert!(antes[0].programa.is_ok());
        assert_eq!(antes[0].corpo, "SET NEW.uf = UPPER(NEW.uf)");
    }

    #[test]
    fn o_ultimo_que_sai_apaga_o_arquivo() {
        let base = dir_temp("apaga");
        let mut r = Rotinas::carregar(&base).unwrap();
        let (_, devido) = r
            .criar_gatilho("loja", gatilho_def("g", "t"), "root")
            .unwrap();
        r.gravar(devido).unwrap();
        let arquivo = base.join("loja").join(ARQUIVO_GATILHOS);
        assert!(arquivo.is_file());
        let devido = r.excluir_gatilho("loja", "g").expect("existia");
        r.gravar(devido).unwrap();
        assert!(!arquivo.exists(), "arquivo vazio devia sumir");
        assert!(r.excluir_gatilho("loja", "g").is_none());
    }

    #[test]
    fn nome_duplicado_recusa() {
        let base = dir_temp("dup");
        let mut r = Rotinas::carregar(&base).unwrap();
        let _ = r
            .criar_gatilho("loja", gatilho_def("g", "t"), "root")
            .unwrap();
        let e = r
            .criar_gatilho("loja", gatilho_def("G", "outra"), "root")
            .unwrap_err();
        assert!(e.to_string().contains("ja existe"), "{e}");
    }

    #[test]
    fn database_que_nao_existe_recusa() {
        let base = dir_temp("semdb");
        let mut r = Rotinas::carregar(&base).unwrap();
        let e = r
            .criar_gatilho("fantasma", gatilho_def("g", "t"), "root")
            .unwrap_err();
        assert!(e.to_string().contains("nao existe"), "{e}");
        // E nome hostil nem chega ao sistema de arquivos.
        let e = r
            .criar_gatilho("../fora", gatilho_def("g", "t"), "root")
            .unwrap_err();
        assert!(e.to_string().contains("nao e um nome"), "{e}");
    }

    /// **Corpo que nao compila mais fica marcado, nao some.** O arquivo pode
    /// ter sido editado a mao; a regra quebrada tem de aparecer quebrada.
    #[test]
    fn corpo_quebrado_carrega_marcado() {
        let base = dir_temp("quebrado");
        let j = r#"{"gatilhos":[{"nome":"g","tabela":"t","quando":"BEFORE",
                     "evento":"INSERT","corpo":"SET NEW.x = FLOOR(1)",
                     "criado_em":"","criado_por":""}]}"#;
        std::fs::write(base.join("loja").join(ARQUIVO_GATILHOS), j).unwrap();
        let r = Rotinas::carregar(&base).unwrap();
        let (antes, _) = r.gatilhos_de("loja", "t", Evento::Inserir);
        assert_eq!(antes.len(), 1);
        let erro = antes[0].programa.as_ref().unwrap_err();
        assert!(erro.contains("FLOOR"), "{erro}");
    }

    /// JSON invalido derruba a carga com o caminho no erro — subir sem as
    /// regras do dono seria pior que nao subir.
    #[test]
    fn json_invalido_derruba_a_carga() {
        let base = dir_temp("torto");
        std::fs::write(base.join("loja").join(ARQUIVO_GATILHOS), "{torto").unwrap();
        let e = Rotinas::carregar(&base).unwrap_err();
        assert!(e.to_string().contains("gatilhos.json"), "{e}");
    }

    #[test]
    fn excluir_da_tabela_leva_so_os_dela() {
        let base = dir_temp("portabela");
        let mut r = Rotinas::carregar(&base).unwrap();
        for (g, tabela) in [("a", "clientes"), ("b", "clientes"), ("c", "vendas")] {
            let (_, devido) = r
                .criar_gatilho("loja", gatilho_def(g, tabela), "root")
                .unwrap();
            r.gravar(devido).unwrap();
        }
        let (saiu, devido) = r.excluir_gatilhos_da_tabela("loja", "clientes");
        assert_eq!(saiu, 2);
        r.gravar(devido.expect("dois sairam")).unwrap();
        assert!(r.ha_gatilhos());
        let (antes, _) = r.gatilhos_de("loja", "vendas", Evento::Inserir);
        assert_eq!(antes.len(), 1);
    }

    #[test]
    fn procedimento_vai_e_volta_com_parametros() {
        let base = dir_temp("proc");
        let mut r = Rotinas::carregar(&base).unwrap();
        let def = ProcedimentoDef {
            nome: "somar".into(),
            parametros: vec![
                Parametro {
                    modo: Modo::Entrada,
                    nome: "ate".into(),
                    tipo: phxsql_sql::rotina::Tipo::Inteiro,
                    tipo_escrito: "INT".into(),
                },
                Parametro {
                    modo: Modo::Saida,
                    nome: "total".into(),
                    tipo: phxsql_sql::rotina::Tipo::Decimal { escala: 2 },
                    tipo_escrito: "DECIMAL(15,2)".into(),
                },
            ],
            corpo: "SET total = ate * 1.00".into(),
        };
        let (_, devido) = r.criar_procedimento("loja", def, "root").unwrap();
        r.gravar(devido).unwrap();
        let r2 = Rotinas::carregar(&base).unwrap();
        let p = r2.procedimento("loja", "SOMAR").expect("procura sem caixa");
        assert_eq!(p.parametros[1].tipo_escrito, "DECIMAL(15,2)");
        assert_eq!(
            p.parametros[1].tipo,
            phxsql_sql::rotina::Tipo::Decimal { escala: 2 }
        );
        assert!(p.programa.is_ok());
    }
}
