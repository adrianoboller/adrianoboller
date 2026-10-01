//! As visoes: o `CREATE VIEW` guardado por database.
//!
//! # Onde mora, e por que no MESMO molde das rotinas
//!
//! Cada database guarda as suas num `visoes.json` no proprio diretorio --
//! exatamente como `gatilhos.json` e `procedimentos.json` (ver
//! [`crate::rotinas`]). JSON e nao formato binario porque isto e CADASTRO e
//! nao dado: muda por comando, se le no olho, e viaja junto com o backup do
//! diretorio. Arquivo ausente = zero visoes = comportamento de sempre; quando
//! a ultima visao de um database sai, o arquivo sai junto, para o ausente
//! continuar significando o que significa.
//!
//! Um molde novo aqui -- um `visoes` dentro do `config.json`, digamos --
//! criaria uma segunda verdade ao lado do diretorio do banco: restaurar o
//! backup de um database traria as tabelas e os gatilhos e deixaria as visoes
//! para tras.
//!
//! # Guarda TEXTO, e o texto e analisado a cada uso
//!
//! Ao contrario dos gatilhos, a visao NAO e compilada na carga. E deliberado,
//! e o motivo e o alvo: um gatilho fala de uma tabela que existe agora, e uma
//! visao fala de uma tabela que pode deixar de existir depois. Guardar o plano
//! compilado o congelaria contra um esquema que envelhece -- a coluna nova nao
//! apareceria, e a tabela apagada so daria erro no dia da carga do servidor.
//!
//! Analisar a cada uso custa um parse por consulta e paga: **visao que aponta
//! para tabela que sumiu recusa NA HORA DE USAR, nomeando a tabela**, que e
//! quando alguem esta olhando.

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

pub const ARQUIVO: &str = "visoes.json";

/// Uma visao guardada.
#[derive(Debug, Clone)]
pub struct Visao {
    pub nome: String,
    /// O SQL como o autor escreveu. Verbatim, so a quem o escreveu ou
    /// administra o database: aos outros a listagem devolve
    /// [`Visao::para_json_redigida`] (pedido 359).
    pub sql: String,
    pub criado_em: String,
    pub criado_por: String,
}

impl Visao {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(&self.nome)),
            ("sql", Json::texto_de(&self.sql)),
            ("criado_em", Json::texto_de(&self.criado_em)),
            ("criado_por", Json::texto_de(&self.criado_por)),
        ])
    }

    /// A visao para quem tem so `ler` no database (pedido 359).
    ///
    /// # Por que o SQL sai REDIGIDO, e nao omitido nem recortado
    ///
    /// O texto guarda o que o autor digitou: `WHERE cpf='52998224725'` traz o
    /// valor, e o comentario (`-- senha: x`) vem junto. Quem so le o catalogo
    /// tem direito a FORMA da consulta -- que tabelas e colunas ela usa --, e
    /// nao ao literal. Recortar dependeria de o literal estar escrito de um
    /// jeito; analisar acha todos. E o MESMO motor do `perfil.txt` do
    /// Profiler ([`phxsql_sql::usuario::normalizado`]): cada literal vira `?`,
    /// o nome entre aspas duplas vira `"***"` (no MySQL(R) aspas duplas sao
    /// texto), o comentario some porque o lexico o descarta, e o que nao se
    /// analisa vira o tamanho em bytes. Uma segunda redacao aqui seria a
    /// decisao escrita duas vezes, e a copia esquecida vazaria.
    pub fn para_json_redigida(&self) -> Json {
        Json::objeto(vec![
            ("nome", Json::texto_de(&self.nome)),
            (
                "sql",
                Json::texto_de(phxsql_sql::usuario::normalizado(&self.sql)),
            ),
            ("redigido", Json::Bool(true)),
            ("criado_em", Json::texto_de(&self.criado_em)),
            ("criado_por", Json::texto_de(&self.criado_por)),
        ])
    }

    fn do_disco(j: &Json, arquivo: &Path) -> Result<Visao> {
        let nome = j.texto_ou("nome", "").trim().to_string();
        let sql = j.texto_ou("sql", "").trim().to_string();
        if nome.is_empty() || sql.is_empty() {
            return Err(PhxError::Esquema(format!(
                "{}: visao sem nome ou sem sql",
                arquivo.display()
            )));
        }
        Ok(Visao {
            nome,
            sql,
            criado_em: j.texto_ou("criado_em", "").to_string(),
            criado_por: j.texto_ou("criado_por", "").to_string(),
        })
    }
}

/// O `visoes.json` de um database que uma mudanca deixou devendo ao disco --
/// pedido 595, o mesmo molde do [`crate::rotinas::PorGravar`]: a mudanca mexe
/// na memoria sob a trava do registro, que a op `sql` toma a cada consulta
/// quando ha visao, e o disco vem fora dela.
#[must_use = "a mudanca so vale no disco depois de Retrato::gravar"]
#[derive(Debug)]
pub struct PorGravar {
    db: String,
}

/// O registro inteiro, uma lista por database que tem alguma visao.
#[derive(Debug)]
pub struct Visoes {
    base: PathBuf,
    dbs: HashMap<String, Vec<Visao>>,
}

impl Visoes {
    /// Carrega tudo que existe embaixo de `base`.
    ///
    /// Um arquivo que nao e JSON valido DERRUBA a subida, com o caminho e o
    /// motivo -- a mesma decisao das rotinas: subir sem as visoes que o dono
    /// escreveu faria toda consulta que as usa responder «tabela nao existe»,
    /// em silencio, como se elas nunca tivessem sido criadas.
    pub fn carregar(base: &Path) -> Result<Visoes> {
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
                let lista = Visoes::carregar_db(&entrada.path())?;
                if !lista.is_empty() {
                    dbs.insert(nome_db, lista);
                }
            }
        }
        Ok(Visoes {
            base: base.to_path_buf(),
            dbs,
        })
    }

    fn carregar_db(dir: &Path) -> Result<Vec<Visao>> {
        let arquivo = dir.join(ARQUIVO);
        if !arquivo.is_file() {
            return Ok(Vec::new());
        }
        let texto = std::fs::read_to_string(&arquivo)?;
        let j = Json::analisar(&texto)
            .map_err(|e| PhxError::Esquema(format!("{}: {e}", arquivo.display())))?;
        let mut saida = Vec::new();
        for v in j.campo("visoes").and_then(Json::lista).unwrap_or(&[]) {
            saida.push(Visao::do_disco(v, &arquivo)?);
        }
        Ok(saida)
    }

    /// Ha alguma visao em algum database?
    ///
    /// E o que abastece o portao atomico do servidor: a op `sql` pergunta isto
    /// ANTES de olhar o nome do `FROM`, e num servidor sem visao nenhuma ela
    /// paga um `bool` e mais nada. O portao que decide se ha trabalho vem
    /// antes do trabalho.
    pub fn ha_visoes(&self) -> bool {
        self.dbs.values().any(|v| !v.is_empty())
    }

    pub fn do_db(&self, db: &str) -> &[Visao] {
        self.dbs.get(db).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn por_nome(&self, db: &str, nome: &str) -> Option<&Visao> {
        self.dbs
            .get(db)?
            .iter()
            .find(|v| v.nome.eq_ignore_ascii_case(nome))
    }

    /// Grava uma visao nova. `substituir` e o `CREATE OR REPLACE`.
    pub fn criar(
        &mut self,
        db: &str,
        nome: &str,
        sql: &str,
        criado_por: &str,
        substituir: bool,
    ) -> Result<(Visao, PorGravar)> {
        self.conferir_nome_do_db(db)?;
        let nome = nome.trim();
        if nome.is_empty() {
            return Err(PhxError::Esquema("informe \"nome\" da visao".into()));
        }
        // O nome vira chave de busca e aparece em `FROM`, entao ele obedece a
        // mesma regra de um nome de tabela -- inclusive a sonda de travessia,
        // porque este nome pode ter vindo de dentro de um texto SQL, que a
        // sonda do `despachar` nao ve.
        if phxsql_store::catalogo::nome_hostil(nome) {
            return Err(PhxError::Autorizacao(format!(
                "visao {nome:?} nao e um nome"
            )));
        }
        let lista = self.dbs.entry(db.to_string()).or_default();
        if let Some(i) = lista.iter().position(|v| v.nome.eq_ignore_ascii_case(nome)) {
            if !substituir {
                return Err(PhxError::Duplicado(format!(
                    "ja existe a visao {nome:?} em {db}; use \"substituir\": true \
                     ou exclua antes"
                )));
            }
            lista.remove(i);
        }
        let v = Visao {
            nome: nome.to_string(),
            sql: sql.trim().to_string(),
            criado_em: phxsql_core::datahora::instante_iso(crate::agora_ms()),
            criado_por: criado_por.to_string(),
        };
        lista.push(v.clone());
        Ok((v, PorGravar { db: db.to_string() }))
    }

    /// Devolve `None` quando a visao nao existia -- e nada a gravar.
    pub fn excluir(&mut self, db: &str, nome: &str) -> Option<PorGravar> {
        let lista = self.dbs.get_mut(db)?;
        let antes = lista.len();
        lista.retain(|v| !v.nome.eq_ignore_ascii_case(nome));
        (lista.len() != antes).then(|| PorGravar { db: db.to_string() })
    }

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

    /// O que o `visoes.json` devido deve conter AGORA -- ver
    /// [`crate::rotinas::Retrato`], o mesmo motor de disco das rotinas.
    pub fn retrato(&self, devido: &PorGravar) -> crate::rotinas::Retrato {
        let arquivo = self.base.join(&devido.db).join(ARQUIVO);
        let lista = self.dbs.get(&devido.db).map(Vec::as_slice).unwrap_or(&[]);
        // Zero visoes apaga o arquivo, para «ausente» continuar querendo dizer
        // «nenhuma» -- um `{"visoes":[]}` no disco seria um terceiro estado
        // que nada le.
        let corpo = (!lista.is_empty()).then(|| {
            Json::objeto(vec![(
                "visoes",
                Json::Lista(lista.iter().map(Visao::para_json).collect()),
            )])
            .escrever_identado()
        });
        crate::rotinas::Retrato::novo(arquivo, corpo)
    }

    /// O retrato tirado e gravado na mesma chamada -- para quem nao divide o
    /// registro com ninguem; o servidor grava fora da trava.
    pub fn gravar(&self, devido: PorGravar) -> Result<()> {
        self.retrato(&devido).gravar()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn base(rotulo: &str) -> DirTemp {
        let d = DirTemp::novo(&format!("visoes-{rotulo}"));
        std::fs::create_dir_all(d.join("b")).unwrap();
        d
    }

    /// Ida e volta pelo disco: o que se grava e o que se le.
    #[test]
    fn a_visao_sobrevive_ao_disco() {
        let d = base("disco");
        let mut v = Visoes::carregar(&d).unwrap();
        assert!(!v.ha_visoes(), "nasceu com visao que ninguem criou");
        let (_, devido) = v
            .criar("b", "v_c", "SELECT * FROM c", "ana", false)
            .unwrap();
        v.gravar(devido).unwrap();

        let outra = Visoes::carregar(&d).unwrap();
        assert!(outra.ha_visoes());
        let lida = outra
            .por_nome("b", "V_C")
            .expect("a visao nao voltou do disco");
        assert_eq!(lida.sql, "SELECT * FROM c");
        assert_eq!(lida.criado_por, "ana");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Nome repetido recusa; `substituir` troca o texto sem duplicar.
    #[test]
    fn nome_repetido_recusa_e_substituir_troca() {
        let d = base("repetido");
        let mut v = Visoes::carregar(&d).unwrap();
        let _ = v.criar("b", "v", "SELECT * FROM c", "ana", false).unwrap();
        let e = v
            .criar("b", "V", "SELECT * FROM outra", "ana", false)
            .expect_err("o nome repetido passou");
        assert!(e.to_string().contains("substituir"), "{e}");
        let _ = v
            .criar("b", "v", "SELECT * FROM outra", "ana", true)
            .unwrap();
        assert_eq!(v.do_db("b").len(), 1);
        assert_eq!(v.por_nome("b", "v").unwrap().sql, "SELECT * FROM outra");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// **A ultima visao que sai leva o arquivo junto.**
    ///
    /// Sem isto, «arquivo ausente = nenhuma visao» deixaria de valer, e um
    /// `{"visoes":[]}` no disco seria um terceiro estado que nada le.
    #[test]
    fn a_ultima_visao_leva_o_arquivo() {
        let d = base("ultima");
        let mut v = Visoes::carregar(&d).unwrap();
        let (_, devido) = v.criar("b", "v", "SELECT * FROM c", "ana", false).unwrap();
        v.gravar(devido).unwrap();
        assert!(d.join("b").join(ARQUIVO).is_file());
        let devido = v.excluir("b", "v").expect("existia");
        v.gravar(devido).unwrap();
        assert!(!d.join("b").join(ARQUIVO).exists(), "o arquivo vazio ficou");
        assert!(v.excluir("b", "v").is_none(), "excluiu o que nao existia");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Nome hostil e database que nao existe recusam, nomeando.
    #[test]
    fn as_recusas_de_nome_nomeiam() {
        let d = base("nome");
        let mut v = Visoes::carregar(&d).unwrap();
        let e = v
            .criar("b", "../fora", "SELECT 1", "ana", false)
            .unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
        let e = v
            .criar("nao_existe", "v", "SELECT 1", "ana", false)
            .unwrap_err();
        assert!(e.to_string().contains("nao_existe"), "{e}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
