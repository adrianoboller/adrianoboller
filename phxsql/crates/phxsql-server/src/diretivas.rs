//! O **diario administrativo**: toda alteracao de diretiva, com os nove campos
//! que o dono pediu.
//!
//! ```text
//! {"data_hora":"2026-09-07 18:40:12,345","servidor":"127.0.0.1:5000",
//!  "banco":"","recurso":"max_linhas","valor_anterior":1000,"valor_novo":500,
//!  "usuario":"ana","ip_origem":"192.168.50.20","motivo":"pico de exportacao"}
//! ```
//!
//! # Por que ARQUIVO, e nao uma tabela do `phxsys`
//!
//! A tabela seria mais bonita — a grade ja a editaria, o backup ja a levaria.
//! E ela esta errada aqui por tres motivos medidos contra este servidor:
//!
//! 1. **O que se audita nao pode desligar a auditoria.** A tabela viveria num
//!    database, e database obedece a `bases_proibidas`, a permissao por base e
//!    ao `somente_leitura` — todos configuraveis pelo mesmo `ALTER SERVER` que
//!    o diario existe para registrar. Trilha que a propria mudanca pode calar
//!    nao e trilha.
//! 2. **A configuracao muda ANTES de haver banco.** Um servidor recem-subido
//!    tem zero databases, e gravar a primeira diretiva criaria o `phxsys` como
//!    efeito colateral de mexer num campo.
//! 3. **O molde da casa ja existe**, e e o `acessos.log`: JSON Lines, uma
//!    linha por evento, legivel a olho nu, `grep`avel, e sobrevive quando o
//!    motor de dados nao esta utilizavel — que e exatamente a hora em que
//!    alguem vai querer saber quem mexeu na configuracao.
//!
//! # O que e mascarado, e por que a regra e por NOME
//!
//! Nenhum campo editavel de hoje carrega credencial — `token`, `rest.token`,
//! `alertas.email.senha` e `cifra.senha` estao de fora do `CAMPOS_EDITAVEIS`
//! de proposito. A mascara existe para o DIA em que alguem acrescentar um, e
//! por isso ela olha o nome do campo em vez de uma lista de campos: lista de
//! segredos envelhece calada, e o primeiro segredo que faltar nela vaza em
//! texto puro num arquivo que ninguem trata como sigiloso.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use phxsql_core::datahora::instante_iso;
use phxsql_core::error::Result;
use phxsql_core::json::Json;

/// O que se escreve no lugar de um valor sigiloso. O mesmo texto que a op
/// `config` ja usa para o token, para nao haver duas convencoes.
pub const OCULTO: &str = "(oculto)";

/// Uma alteracao administrativa, com os nove campos do pedido.
#[derive(Debug, Clone, PartialEq)]
pub struct Alteracao {
    /// Milissegundos desde a epoca Unix. Vira `data_hora` legivel na gravacao.
    pub quando_ms: i64,
    /// Qual servidor — o `bind` configurado, que e o que identifica o no num
    /// cluster com quatro deles.
    pub servidor: String,
    /// O banco, quando a diretiva e de banco. Vazio no escopo de servidor.
    pub banco: String,
    /// O campo mudado, pelo nome do `config.json`: `recursos.cache_paginas`.
    pub recurso: String,
    pub valor_anterior: Json,
    pub valor_novo: Json,
    /// Login de quem mandou. `(token de servico)` quando nao houve login.
    pub usuario: String,
    pub ip_origem: String,
    pub motivo: String,
}

impl Alteracao {
    pub fn data_hora(&self) -> String {
        instante_iso(self.quando_ms)
    }

    /// A linha do diario, com os valores ja mascarados quando o campo e
    /// sigiloso.
    pub fn para_json(&self) -> Json {
        let esconder = campo_sigiloso(&self.recurso);
        let valor = |v: &Json| {
            if esconder {
                Json::texto_de(OCULTO)
            } else {
                v.clone()
            }
        };
        Json::objeto(vec![
            ("data_hora", Json::texto_de(self.data_hora())),
            ("quando_ms", Json::Numero(self.quando_ms as f64)),
            ("servidor", Json::texto_de(&self.servidor)),
            ("banco", Json::texto_de(&self.banco)),
            ("recurso", Json::texto_de(&self.recurso)),
            ("valor_anterior", valor(&self.valor_anterior)),
            ("valor_novo", valor(&self.valor_novo)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("ip_origem", Json::texto_de(&self.ip_origem)),
            ("motivo", Json::texto_de(&self.motivo)),
        ])
    }

    fn de_json(j: &Json) -> Option<Alteracao> {
        Some(Alteracao {
            quando_ms: j.campo("quando_ms")?.numero()? as i64,
            servidor: j.texto_ou("servidor", "").to_string(),
            banco: j.texto_ou("banco", "").to_string(),
            recurso: j.texto_ou("recurso", "").to_string(),
            valor_anterior: j.campo("valor_anterior").cloned().unwrap_or(Json::Nulo),
            valor_novo: j.campo("valor_novo").cloned().unwrap_or(Json::Nulo),
            usuario: j.texto_ou("usuario", "").to_string(),
            ip_origem: j.texto_ou("ip_origem", "").to_string(),
            motivo: j.texto_ou("motivo", "").to_string(),
        })
    }
}

/// Este campo carrega segredo?
///
/// Olha o nome inteiro, em minusculas — `alertas.email.senha`, `rest.token`,
/// `cifra_fio.chave_privada_env`. Falso positivo aqui custa um valor escondido
/// no diario; falso negativo custa uma credencial em texto puro num arquivo de
/// log. Os dois lados nao valem o mesmo, e a duvida vai para o lado de esconder.
pub fn campo_sigiloso(campo: &str) -> bool {
    let c = campo.to_ascii_lowercase();
    ["token", "senha", "password", "secret", "chave_privada"]
        .iter()
        .any(|s| c.contains(s))
}

/// O diario, aberto para acrescentar.
pub struct Diario {
    caminho: PathBuf,
}

impl Diario {
    /// Onde o diario mora: ao lado do `acessos.log`, com o nome
    /// `diretivas.log`.
    ///
    /// Sai do caminho do log de acessos, e nao de um campo proprio do
    /// `config.json`, porque um caminho a mais e um caminho a mais para
    /// alguem esquecer de apontar — e um diario gravado no diretorio de
    /// trabalho do processo e um diario perdido.
    pub fn ao_lado_de(log_acessos: &Path) -> Diario {
        let caminho = match log_acessos.parent().filter(|d| !d.as_os_str().is_empty()) {
            Some(dir) => dir.join("diretivas.log"),
            None => PathBuf::from("diretivas.log"),
        };
        Diario { caminho }
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Grava e descarrega na hora, pelo mesmo motivo do `acessos.log`: diario
    /// que se perde no buffer quando o processo cai nao serve para nada — e
    /// mudanca de configuracao e justamente o que costuma preceder uma queda.
    pub fn registrar(&self, a: &Alteracao) -> Result<()> {
        if let Some(dir) = self.caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let mut arquivo = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.caminho)?;
        writeln!(arquivo, "{}", a.para_json().escrever())?;
        arquivo.flush()?;
        Ok(())
    }

    /// As `quantas` ultimas linhas, da mais recente para a mais antiga.
    ///
    /// Linha ilegivel e pulada — um diario truncado por falta de disco ainda
    /// deve ser lido, e e o pedaco legivel que interessa.
    pub fn ultimas(&self, quantas: usize) -> Vec<Alteracao> {
        if quantas == 0 || !self.caminho.exists() {
            return Vec::new();
        }
        let Ok(arquivo) = std::fs::File::open(&self.caminho) else {
            return Vec::new();
        };
        let mut todas: Vec<Alteracao> = BufReader::new(arquivo)
            .lines()
            .map_while(std::result::Result::ok)
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| Json::analisar(&l).ok())
            .filter_map(|j| Alteracao::de_json(&j))
            .collect();
        todas.reverse();
        todas.truncate(quantas);
        todas
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    fn alteracao(recurso: &str, de: Json, para: Json) -> Alteracao {
        Alteracao {
            quando_ms: 1_700_000_000_000,
            servidor: "127.0.0.1:5000".into(),
            banco: String::new(),
            recurso: recurso.into(),
            valor_anterior: de,
            valor_novo: para,
            usuario: "ana".into(),
            ip_origem: "192.168.50.20".into(),
            motivo: "pico de exportacao".into(),
        }
    }

    #[test]
    fn os_nove_campos_estao_todos_na_linha() {
        let a = alteracao("max_linhas", Json::Numero(1000.0), Json::Numero(500.0));
        let j = a.para_json();
        for campo in [
            "data_hora",
            "servidor",
            "banco",
            "recurso",
            "valor_anterior",
            "valor_novo",
            "usuario",
            "ip_origem",
            "motivo",
        ] {
            assert!(
                j.campo(campo).is_some(),
                "faltou o campo {campo:?} do pedido do dono na linha do diario"
            );
        }
        assert_eq!(j.texto_ou("data_hora", ""), "2023-11-14 22:13:20,000");
        assert_eq!(j.campo("valor_anterior").unwrap().numero(), Some(1000.0));
        assert_eq!(j.campo("valor_novo").unwrap().numero(), Some(500.0));
    }

    /// **Senha nunca em texto puro** — nem no diario.
    ///
    /// A prova e nos DOIS sentidos: o campo comum sai como veio, e so o
    /// sigiloso vira `(oculto)`. Uma mascara que escondesse tudo protegeria
    /// igual e deixaria o diario inutil.
    #[test]
    fn o_valor_do_campo_sigiloso_nao_vai_para_o_diario() {
        for campo in [
            "token",
            "rest.token",
            "alertas.email.senha",
            "cifra.senha_env",
            "cifra_fio.chave_privada_env",
        ] {
            let a = alteracao(
                campo,
                Json::texto_de("segredo-velho"),
                Json::texto_de("segredo-novo"),
            );
            let linha = a.para_json().escrever();
            assert!(
                !linha.contains("segredo-velho") && !linha.contains("segredo-novo"),
                "{campo:?} vazou o valor no diario: {linha}"
            );
            assert!(linha.contains(OCULTO), "{campo:?}: {linha}");
        }
        // O controle positivo: campo comum sai INTEIRO, senao a varredura
        // acima estaria provando so que o escritor nao escreve nada.
        let a = alteracao(
            "backup.destino",
            Json::texto_de("/var/velho"),
            Json::texto_de("/var/novo"),
        );
        let linha = a.para_json().escrever();
        assert!(
            linha.contains("/var/velho") && linha.contains("/var/novo"),
            "{linha}"
        );
    }

    #[test]
    fn grava_e_le_da_mais_recente_para_a_mais_antiga() {
        let d = DirTemp::novo("diario-basico");
        let diario = Diario::ao_lado_de(&d.join("acessos.log"));
        assert_eq!(diario.caminho(), d.join("diretivas.log"));
        for n in 1..=3 {
            let mut a = alteracao("max_linhas", Json::Numero(0.0), Json::Numero(n as f64));
            a.quando_ms += n * 1000;
            diario.registrar(&a).unwrap();
        }
        let lidas = diario.ultimas(2);
        assert_eq!(lidas.len(), 2);
        assert_eq!(
            lidas[0].valor_novo.numero(),
            Some(3.0),
            "a mais recente vem primeiro"
        );
        assert_eq!(lidas[1].valor_novo.numero(), Some(2.0));
        assert_eq!(lidas[0].usuario, "ana");
        assert_eq!(lidas[0].ip_origem, "192.168.50.20");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn diario_que_ainda_nao_existe_le_vazio_em_vez_de_quebrar() {
        let d = DirTemp::novo("diario-ausente");
        let diario = Diario::ao_lado_de(&d.join("acessos.log"));
        assert!(diario.ultimas(10).is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
