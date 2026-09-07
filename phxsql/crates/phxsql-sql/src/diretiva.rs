//! As DIRETIVAS do servidor, do banco, da tabela e da conexao — `SHOW … SETTINGS`
//! e `ALTER … SET`.
//!
//! ```text
//! SHOW SERVER SETTINGS;
//! SHOW DATABASE erp SETTINGS;
//! SHOW TABLE clientes SETTINGS;
//! SHOW CONNECTION SETTINGS;
//!
//! ALTER SERVER   SET max_linhas = 500 MOTIVO 'pico de exportacao';
//! ALTER DATABASE erp SET comandos_proibidos = (reindexar, excluir_tabela);
//! ALTER TABLE    clientes SET duplicate_check = TRUE;
//! ALTER CONNECTION SET compression = TRUE;
//! ```
//!
//! # Por que uma gramatica, e nao doze funcoes `HSet…`
//!
//! O HFSQL espalha a configuracao por `HSetServer`, `HSetTransaction`,
//! `HSetLog`, `HSetIntegrity`, `HSetDuplicates`, `HSetTrigger`, propriedades
//! da `Connection` e `HManageTask` — oito portas para a mesma pergunta «o que
//! esta ligado aqui?». O pedido do dono foi centralizar: **um verbo para ver,
//! um verbo para mudar, e o escopo dito por extenso**.
//!
//! # Por que ele nao passa pelo `sintaxe.rs`
//!
//! Pelo mesmo motivo do [`crate::transacao`]: nao e consulta. Nao tem `FROM`,
//! nao produz linha, nao depende de esquema. E comando de ADMINISTRACAO, e
//! vira um pedido do protocolo — o mesmo caminho, o mesmo portao.
//!
//! # O que ele NAO rouba
//!
//! `SHOW TRIGGERS`, `SHOW PROCEDURES` e `SHOW PROCEDURE STATUS` continuam do
//! [`crate::rotina`]. Este modulo so reclama a frase quando a palavra depois
//! do `SHOW` e `SERVER`, `DATABASE`, `TABLE` ou `CONNECTION` — quatro palavras
//! que o `rotina` nunca atendeu. Reclamar ali e o que permite dizer «faltou
//! SETTINGS» em vez de deixar o outro analisador responder «SHOW nesta camada
//! lista TRIGGERS ou PROCEDURES», que manda quem digitou procurar no lugar
//! errado.

use phxsql_core::error::Result;
use phxsql_core::json::Json;

use crate::lexico::{self, Simbolo, Token};

/// Sobre o que a diretiva fala.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escopo {
    Servidor,
    Base,
    Tabela,
    Conexao,
}

impl Escopo {
    /// O nome que viaja no campo `"escopo"` do pedido.
    pub fn nome(self) -> &'static str {
        match self {
            Escopo::Servidor => "servidor",
            Escopo::Base => "database",
            Escopo::Tabela => "tabela",
            Escopo::Conexao => "conexao",
        }
    }

    fn da_palavra(p: &str) -> Option<Escopo> {
        Some(match p {
            "SERVER" | "SERVIDOR" => Escopo::Servidor,
            "DATABASE" | "SCHEMA" | "BANCO" => Escopo::Base,
            "TABLE" | "TABELA" => Escopo::Tabela,
            "CONNECTION" | "CONEXAO" => Escopo::Conexao,
            _ => return None,
        })
    }
}

/// Um comando de diretiva ja reconhecido.
#[derive(Debug, Clone, PartialEq)]
pub struct Comando {
    /// `"diretivas"` (o `SHOW`) ou `"diretiva_gravar"` (o `ALTER`).
    pub op: String,
    pub escopo: Escopo,
    /// O nome do banco ou da tabela. Vazio no servidor e na conexao.
    pub alvo: String,
    /// O campo, com o ponto quando ele tem secao: `recursos.cache_paginas`.
    pub campo: String,
    pub valor: Json,
    /// O `MOTIVO '…'`, que alimenta o diario administrativo.
    pub motivo: String,
}

impl Comando {
    /// O pedido pronto para o despachar, com os nomes do protocolo.
    ///
    /// O `SHOW SERVER SETTINGS` ja pede o diario junto, e isso e decisao: a
    /// pergunta que uma pessoa faz ao ver uma diretiva estranha e «quem mexeu
    /// nisso?», e um diario que so se le por outro comando e um diario que
    /// ninguem le.
    pub fn pedido(&self) -> Json {
        let mut pares: Vec<(String, Json)> = vec![
            ("escopo".into(), Json::texto_de(self.escopo.nome())),
            ("_sql".into(), Json::Bool(true)),
        ];
        match self.escopo {
            Escopo::Base => pares.push(("database".into(), Json::texto_de(&self.alvo))),
            Escopo::Tabela => pares.push(("tabela".into(), Json::texto_de(&self.alvo))),
            _ => {}
        }
        if self.op == "diretivas" {
            pares.push(("diario".into(), Json::de_u64(DIARIO_NO_SHOW)));
        } else {
            pares.push(("campo".into(), Json::texto_de(&self.campo)));
            pares.push(("valor".into(), self.valor.clone()));
            pares.push(("motivo".into(), Json::texto_de(&self.motivo)));
        }
        Json::Objeto(pares)
    }
}

/// Quantas linhas do diario o `SHOW … SETTINGS` traz junto.
pub const DIARIO_NO_SHOW: u64 = 20;

/// Reconhece um comando de diretiva. `None` quando o texto e outra coisa.
pub fn comando(texto: &str) -> Result<Option<Comando>> {
    // O portao vem ANTES do trabalho: quem nao comeca por SHOW ou ALTER nao
    // paga nem o lexico. E o mesmo cuidado do detector de transacao — este
    // modulo e consultado em TODO comando SQL que chega.
    if !primeira_palavra_e_de_diretiva(texto) {
        return Ok(None);
    }
    let Ok(simbolos) = lexico::analisar(texto) else {
        return Ok(None);
    };
    let mut p = Passo { s: &simbolos, i: 0 };
    let pos = simbolos.first().map(|x| x.posicao).unwrap_or(0);
    let verbo = p.palavra();
    p.i += 1;

    // A palavra do escopo decide se a frase e nossa. `SHOW TRIGGERS` sai por
    // aqui com `None` e vai para o `rotina`, inteiro.
    let Some(escopo) = Escopo::da_palavra(&p.palavra()) else {
        return Ok(None);
    };
    p.i += 1;

    let alvo = match escopo {
        Escopo::Base | Escopo::Tabela => p.exigir_nome_qualificado(pos, escopo)?,
        _ => String::new(),
    };

    if verbo == "SHOW" {
        p.exigir_palavra(pos, "SETTINGS", escopo)?;
        p.fim(pos, "SHOW")?;
        return Ok(Some(Comando {
            op: "diretivas".into(),
            escopo,
            alvo,
            campo: String::new(),
            valor: Json::Nulo,
            motivo: String::new(),
        }));
    }

    p.exigir_palavra(pos, "SET", escopo)?;
    let campo = p.exigir_campo(pos)?;
    p.exigir_igual(pos, &campo)?;
    let valor = p.exigir_valor(pos, &campo)?;
    let motivo = p.motivo(pos)?;
    p.fim(pos, "ALTER")?;
    Ok(Some(Comando {
        op: "diretiva_gravar".into(),
        escopo,
        alvo,
        campo,
        valor,
        motivo,
    }))
}

/// O texto comeca por `SHOW` ou `ALTER`?
///
/// Le so a primeira palavra, sem lexico — o portao que decide se vale a pena
/// analisar o texto vem antes do trabalho.
fn primeira_palavra_e_de_diretiva(texto: &str) -> bool {
    let primeira: String = texto
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    matches!(primeira.to_ascii_uppercase().as_str(), "SHOW" | "ALTER")
}

struct Passo<'a> {
    s: &'a [Simbolo],
    i: usize,
}

impl Passo<'_> {
    fn palavra(&self) -> String {
        self.s
            .get(self.i)
            .and_then(|x| x.token.palavra_chave())
            .unwrap_or_default()
    }

    fn nome(&self) -> Option<String> {
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Palavra { texto, .. }) => Some(texto.clone()),
            _ => None,
        }
    }

    fn exigir_palavra(&mut self, pos: usize, esperada: &str, escopo: Escopo) -> Result<()> {
        if self.palavra() != esperada {
            let veio = self
                .s
                .get(self.i)
                .map(|x| x.token.descrever())
                .unwrap_or_else(|| "o fim do comando".into());
            return Err(lexico::erro(
                pos,
                &format!(
                    "esperava {esperada} depois do escopo {}, veio {veio:?}",
                    escopo.nome()
                ),
            ));
        }
        self.i += 1;
        Ok(())
    }

    /// `erp`, `clientes`, `vendas.clientes` — o nome qualificado e o que o
    /// protocolo usa no campo `tabela`.
    fn exigir_nome_qualificado(&mut self, pos: usize, escopo: Escopo) -> Result<String> {
        let mut nome = self.nome().ok_or_else(|| {
            lexico::erro(pos, &format!("esperava o nome depois de {}", escopo.nome()))
        })?;
        self.i += 1;
        while matches!(self.s.get(self.i).map(|x| &x.token), Some(Token::Ponto)) {
            self.i += 1;
            let parte = self.nome().ok_or_else(|| {
                lexico::erro(pos, "esperava o nome depois do ponto (schema.tabela)")
            })?;
            self.i += 1;
            nome.push('.');
            nome.push_str(&parte);
        }
        Ok(nome)
    }

    /// O campo, que pode ter secao: `recursos.cache_paginas`.
    ///
    /// Junta as partes com ponto de proposito — e exatamente a forma que o
    /// `config_gravar` ja aceita, e e o que faz o `ALTER SERVER` cair no
    /// mesmo caminho sem tradutor no meio.
    fn exigir_campo(&mut self, pos: usize) -> Result<String> {
        let mut campo = self
            .nome()
            .ok_or_else(|| lexico::erro(pos, "esperava o nome do campo depois de SET"))?;
        self.i += 1;
        while matches!(self.s.get(self.i).map(|x| &x.token), Some(Token::Ponto)) {
            self.i += 1;
            let parte = self
                .nome()
                .ok_or_else(|| lexico::erro(pos, "esperava o nome depois do ponto no campo"))?;
            self.i += 1;
            campo.push('.');
            campo.push_str(&parte);
        }
        Ok(campo)
    }

    fn exigir_igual(&mut self, pos: usize, campo: &str) -> Result<()> {
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Comparador(lexico::Comparador::Igual)) => {
                self.i += 1;
                Ok(())
            }
            _ => Err(lexico::erro(pos, &format!("esperava = depois de {campo}"))),
        }
    }

    /// `TRUE`/`FALSE`/`ON`/`OFF`, numero, texto entre aspas, palavra solta, ou
    /// uma lista entre parenteses.
    ///
    /// # Por que `ON`/`OFF` valem
    ///
    /// Porque o dono escreveu `TRUE`/`FALSE` e todo administrador que veio de
    /// outro banco escreve `ON`/`OFF`. Aceitar os dois custa uma linha; recusar
    /// custa uma pessoa procurando no manual qual das duas grafias e a certa.
    fn exigir_valor(&mut self, pos: usize, campo: &str) -> Result<Json> {
        // A lista vem primeiro porque `(` nao e valor de nada mais.
        if matches!(self.s.get(self.i).map(|x| &x.token), Some(Token::AbreParen)) {
            self.i += 1;
            let mut itens = Vec::new();
            // `()` e lista vazia, e ela chega ate o servidor: quem recusa e
            // quem sabe o que a lista vazia significaria naquele campo.
            if !matches!(
                self.s.get(self.i).map(|x| &x.token),
                Some(Token::FechaParen)
            ) {
                loop {
                    itens.push(self.item_da_lista(pos, campo)?);
                    match self.s.get(self.i).map(|x| &x.token) {
                        Some(Token::Virgula) => self.i += 1,
                        _ => break,
                    }
                }
            }
            if !matches!(
                self.s.get(self.i).map(|x| &x.token),
                Some(Token::FechaParen)
            ) {
                return Err(lexico::erro(
                    pos,
                    &format!("faltou fechar o parentese da lista de {campo}"),
                ));
            }
            self.i += 1;
            return Ok(Json::Lista(itens));
        }
        // `-1` chega em dois simbolos, e ha campo que aceita negativo.
        let negativo = matches!(self.s.get(self.i).map(|x| &x.token), Some(Token::Menos));
        if negativo {
            self.i += 1;
        }
        let token = self.s.get(self.i).map(|x| x.token.clone());
        let valor = match token {
            Some(Token::Numero(n)) => {
                let n: f64 = n.parse().map_err(|_| {
                    lexico::erro(pos, &format!("{n:?} nao e um numero que eu saiba ler"))
                })?;
                Json::Numero(if negativo { -n } else { n })
            }
            _ if negativo => {
                return Err(lexico::erro(
                    pos,
                    &format!("o sinal de menos em {campo} tem de vir antes de um numero"),
                ))
            }
            Some(Token::Texto(t)) => Json::texto_de(t),
            Some(Token::Palavra { texto, citado }) => {
                match (citado, texto.to_ascii_uppercase().as_str()) {
                    (false, "TRUE" | "ON" | "YES" | "SIM" | "LIGADO") => Json::Bool(true),
                    (false, "FALSE" | "OFF" | "NO" | "NAO" | "DESLIGADO") => Json::Bool(false),
                    (false, "NULL" | "NULO") => Json::Nulo,
                    // Palavra solta e TEXTO — `recursos.durabilidade =
                    // por_lote` e o caso que existe.
                    _ => Json::texto_de(texto),
                }
            }
            _ => {
                return Err(lexico::erro(
                    pos,
                    &format!(
                        "esperava o valor de {campo}: TRUE, FALSE, um numero, \
                         um texto entre aspas ou uma lista entre parenteses"
                    ),
                ))
            }
        };
        self.i += 1;
        Ok(valor)
    }

    /// Um item de lista: sempre TEXTO, porque as listas deste servidor
    /// (`rest.tabelas`, `cifra.tabelas`, `comandos_proibidos`) sao de nomes.
    fn item_da_lista(&mut self, pos: usize, campo: &str) -> Result<Json> {
        let item = match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Texto(t)) => t.clone(),
            Some(Token::Palavra { texto, .. }) => texto.clone(),
            _ => {
                return Err(lexico::erro(
                    pos,
                    &format!("esperava um nome na lista de {campo}"),
                ))
            }
        };
        self.i += 1;
        Ok(Json::texto_de(item))
    }

    /// `MOTIVO '…'` ou `COMMENT '…'`, facultativo.
    ///
    /// Facultativo porque exigir motivo em toda mudanca ensina a escrever
    /// «x» — e um diario cheio de «x» e pior que um diario com o campo vazio,
    /// que ao menos nao mente.
    fn motivo(&mut self, pos: usize) -> Result<String> {
        if !matches!(self.palavra().as_str(), "MOTIVO" | "COMMENT" | "COMENTARIO") {
            return Ok(String::new());
        }
        let rotulo = self.palavra();
        self.i += 1;
        match self.s.get(self.i).map(|x| &x.token) {
            Some(Token::Texto(t)) => {
                let t = t.clone();
                self.i += 1;
                Ok(t)
            }
            _ => Err(lexico::erro(
                pos,
                &format!("esperava o texto entre aspas depois de {rotulo}"),
            )),
        }
    }

    /// Sobra de texto e erro, e nao silencio — a mesma regra do `COMMIT AND
    /// CHAIN`: quem escreveu algo que este comando nao entende precisa saber.
    fn fim(&self, pos: usize, verbo: &str) -> Result<()> {
        let sobra: Vec<&Simbolo> = self.s[self.i.min(self.s.len())..]
            .iter()
            .filter(|x| !matches!(x.token, Token::PontoEVirgula))
            .collect();
        if sobra.is_empty() {
            return Ok(());
        }
        Err(lexico::erro(
            pos,
            &format!(
                "sobrou {:?} depois do comando de {verbo}",
                sobra[0].token.descrever()
            ),
        ))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn c(texto: &str) -> Comando {
        comando(texto)
            .unwrap_or_else(|e| panic!("{texto:?} nao analisou: {e}"))
            .unwrap_or_else(|| panic!("{texto:?} nao foi reconhecido como diretiva"))
    }

    #[test]
    fn os_quatro_show() {
        let s = c("SHOW SERVER SETTINGS");
        assert_eq!(s.op, "diretivas");
        assert_eq!(s.escopo, Escopo::Servidor);
        assert!(s.alvo.is_empty());

        let b = c("SHOW DATABASE erp SETTINGS");
        assert_eq!(b.escopo, Escopo::Base);
        assert_eq!(b.alvo, "erp");

        let t = c("SHOW TABLE vendas.clientes SETTINGS;");
        assert_eq!(t.escopo, Escopo::Tabela);
        assert_eq!(t.alvo, "vendas.clientes");

        assert_eq!(c("SHOW CONNECTION SETTINGS").escopo, Escopo::Conexao);
    }

    /// **O que este modulo NAO rouba.** `SHOW TRIGGERS` e do `rotina`, e
    /// roubar essa frase deixaria o servidor sem listar gatilho nenhum.
    #[test]
    fn nao_rouba_o_show_do_rotina() {
        for texto in [
            "SHOW TRIGGERS",
            "SHOW PROCEDURES",
            "SHOW PROCEDURE STATUS",
            "SELECT * FROM clientes",
            "BEGIN",
            "CREATE PROCEDURE p() BEGIN END",
        ] {
            assert_eq!(
                comando(texto).unwrap(),
                None,
                "{texto:?} foi roubado pelo detector de diretivas"
            );
        }
    }

    #[test]
    fn alter_server_com_os_tres_tipos() {
        let n = c("ALTER SERVER SET max_linhas = 500");
        assert_eq!(n.campo, "max_linhas");
        assert_eq!(n.valor, Json::Numero(500.0));
        assert_eq!(n.escopo, Escopo::Servidor);

        assert_eq!(
            c("ALTER SERVER SET somente_leitura = TRUE").valor,
            Json::Bool(true)
        );
        assert_eq!(
            c("ALTER SERVER SET somente_leitura = FALSE").valor,
            Json::Bool(false)
        );
        // ON/OFF sao os mesmos dois, e quem vem de outro banco escreve assim.
        assert_eq!(c("ALTER SERVER SET espelho = ON").valor, Json::Bool(true));
        assert_eq!(c("ALTER SERVER SET espelho = OFF").valor, Json::Bool(false));

        assert_eq!(
            c("ALTER SERVER SET backup.hora = '03:00'").valor,
            Json::texto_de("03:00")
        );
        // Palavra solta e texto: e o `recursos.durabilidade` de verdade.
        assert_eq!(
            c("ALTER SERVER SET recursos.durabilidade = por_lote").valor,
            Json::texto_de("por_lote")
        );
    }

    #[test]
    fn o_campo_com_secao_chega_com_o_ponto() {
        let x = c("ALTER SERVER SET recursos.cache_paginas = 4096");
        assert_eq!(x.campo, "recursos.cache_paginas");
        assert_eq!(x.valor, Json::Numero(4096.0));
    }

    #[test]
    fn a_lista_entre_parenteses() {
        let x = c("ALTER DATABASE erp SET comandos_proibidos = (reindexar, excluir_tabela)");
        assert_eq!(x.escopo, Escopo::Base);
        assert_eq!(x.alvo, "erp");
        assert_eq!(
            x.valor,
            Json::Lista(vec![
                Json::texto_de("reindexar"),
                Json::texto_de("excluir_tabela")
            ])
        );
        // Entre aspas tambem, que e o que um cliente monta por concatenacao.
        assert_eq!(
            c("ALTER DATABASE erp SET comandos_proibidos = ('reindexar')").valor,
            Json::Lista(vec![Json::texto_de("reindexar")])
        );
    }

    #[test]
    fn o_motivo_alimenta_o_diario() {
        let x = c("ALTER SERVER SET max_linhas = 500 MOTIVO 'pico de exportacao'");
        assert_eq!(x.motivo, "pico de exportacao");
        // COMMENT e sinonimo.
        assert_eq!(
            c("ALTER SERVER SET max_linhas = 500 COMMENT 'idem'").motivo,
            "idem"
        );
        // E ele e FACULTATIVO.
        assert!(c("ALTER SERVER SET max_linhas = 500").motivo.is_empty());
    }

    #[test]
    fn o_pedido_leva_escopo_alvo_e_motivo() {
        let p = c("ALTER DATABASE erp SET comandos_proibidos = (reindexar) MOTIVO 'auditoria'")
            .pedido();
        assert_eq!(p.texto_ou("escopo", ""), "database");
        assert_eq!(p.texto_ou("database", ""), "erp");
        assert_eq!(p.texto_ou("campo", ""), "comandos_proibidos");
        assert_eq!(p.texto_ou("motivo", ""), "auditoria");

        let s = c("SHOW SERVER SETTINGS").pedido();
        assert_eq!(s.texto_ou("escopo", ""), "servidor");
        assert_eq!(s.inteiro_ou("diario", 0), DIARIO_NO_SHOW as i64);
    }

    /// A frase incompleta recusa DIZENDO o que falta, em vez de cair no
    /// analisador de `SELECT` e voltar «esperava FROM».
    #[test]
    fn o_que_falta_recusa_dizendo_o_que_falta() {
        for (texto, pedaco) in [
            ("SHOW SERVER", "esperava SETTINGS"),
            ("SHOW DATABASE erp", "esperava SETTINGS"),
            ("ALTER SERVER max_linhas = 1", "esperava SET"),
            ("ALTER SERVER SET max_linhas", "esperava = depois"),
            ("ALTER SERVER SET max_linhas =", "esperava o valor"),
            ("ALTER DATABASE SET x = 1", "esperava SET"),
            ("SHOW SERVER SETTINGS AND MORE", "sobrou"),
            (
                "ALTER SERVER SET x = 1 MOTIVO",
                "esperava o texto entre aspas",
            ),
        ] {
            let e = comando(texto).expect_err(&format!("{texto:?} devia recusar"));
            assert!(
                e.to_string().contains(pedaco),
                "{texto:?} recusou com {e}, e eu esperava {pedaco:?}"
            );
        }
    }

    /// O escopo tambem se escreve em portugues — quem digita nao devia ter de
    /// lembrar em que lingua esta a palavra reservada.
    #[test]
    fn os_escopos_em_portugues() {
        assert_eq!(c("SHOW SERVIDOR SETTINGS").escopo, Escopo::Servidor);
        assert_eq!(c("SHOW BANCO erp SETTINGS").escopo, Escopo::Base);
        assert_eq!(c("SHOW TABELA t SETTINGS").escopo, Escopo::Tabela);
        assert_eq!(c("SHOW CONEXAO SETTINGS").escopo, Escopo::Conexao);
    }

    #[test]
    fn o_negativo_chega_negativo() {
        assert_eq!(
            c("ALTER SERVER SET recursos.diario_volume_mib = -1").valor,
            Json::Numero(-1.0)
        );
        let e = comando("ALTER SERVER SET x = -TRUE").expect_err("menos antes de TRUE");
        assert!(e.to_string().contains("antes de um numero"), "{e}");
    }
}
