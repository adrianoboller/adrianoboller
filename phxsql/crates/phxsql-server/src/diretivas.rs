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
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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
    /// Teto de bytes por arquivo. Zero = nao rodizia -- o comportamento de
    /// sempre, e o padrao de quem nao configurou o campo novo do pedido 228
    /// (`diretivas.arquivo_mib`). A logica de girar e a MESMA do Profiler --
    /// ver `crate::rodizio`, para onde ela foi extraida.
    ///
    /// Atomico, e nao um campo comum: o `registrar` abaixo NAO guarda
    /// descritor entre chamadas -- abre, escreve e fecha a cada diretiva,
    /// porque uma alteracao de configuracao e rara. Sem "arquivo corrente"
    /// nenhum para proteger, so estes numeros precisam mudar depois de
    /// construido (em `ALTER SERVER SET`), e um atomico evita pedir `&mut
    /// self` -- e por tabela, um `Mutex<Diario>` novo -- so para isso.
    teto_do_arquivo: AtomicU64,
    /// Quantos arquivos ANTIGOS guardar, alem do corrente.
    manter: AtomicUsize,
    /// Quantas vezes o arquivo virou desde que o servidor subiu.
    rodizios: AtomicU64,
    /// Rodizios que nao deram certo -- renomear ou reabrir falhou.
    falhas_de_rodizio: AtomicU64,
}

impl Diario {
    /// Onde o diario mora: ao lado do `acessos.log`, com o nome
    /// `diretivas.log`.
    ///
    /// Sai do caminho do log de acessos, e nao de um campo proprio do
    /// `config.json`, porque um caminho a mais e um caminho a mais para
    /// alguem esquecer de apontar — e um diario gravado no diretorio de
    /// trabalho do processo e um diario perdido.
    ///
    /// Nasce SEM rodizio (`teto_do_arquivo: 0`) -- quem quiser liga com
    /// [`Diario::definir_rodizio`].
    pub fn ao_lado_de(log_acessos: &Path) -> Diario {
        let caminho = match log_acessos.parent().filter(|d| !d.as_os_str().is_empty()) {
            Some(dir) => dir.join("diretivas.log"),
            None => PathBuf::from("diretivas.log"),
        };
        Diario {
            caminho,
            teto_do_arquivo: AtomicU64::new(0),
            manter: AtomicUsize::new(0),
            rodizios: AtomicU64::new(0),
            falhas_de_rodizio: AtomicU64::new(0),
        }
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// Ajusta o rodizio. Vale para o arquivo CORRENTE, como no Profiler
    /// (`Profiler::definir_rodizio`) e no `LogAcessos`: quem baixou o teto
    /// na tela quer o efeito agora, e nao no proximo arranque.
    pub fn definir_rodizio(&self, teto_do_arquivo: u64, manter: usize) {
        self.teto_do_arquivo
            .store(teto_do_arquivo, Ordering::Relaxed);
        self.manter.store(
            manter.min(crate::profiler::MAX_ARQUIVOS_ANTIGOS),
            Ordering::Relaxed,
        );
    }

    pub fn teto_do_arquivo(&self) -> u64 {
        self.teto_do_arquivo.load(Ordering::Relaxed)
    }

    pub fn manter(&self) -> usize {
        self.manter.load(Ordering::Relaxed)
    }

    pub fn rodizios(&self) -> u64 {
        self.rodizios.load(Ordering::Relaxed)
    }

    pub fn falhas_de_rodizio(&self) -> u64 {
        self.falhas_de_rodizio.load(Ordering::Relaxed)
    }

    /// Grava e descarrega na hora, pelo mesmo motivo do `acessos.log`: diario
    /// que se perde no buffer quando o processo cai nao serve para nada — e
    /// mudanca de configuracao e justamente o que costuma preceder uma queda.
    ///
    /// Confere ANTES de escrever se a linha estoura o teto -- mesma ordem do
    /// `profiler::girar_se_encheu`. Sem descritor persistente, o tamanho
    /// ATUAL vem do disco (`metadata`) em vez de um contador em memoria: nao
    /// ha sessao para acumular, e uma diretiva e rara o bastante para o
    /// `stat()` extra nao custar nada que importe.
    pub fn registrar(&self, a: &Alteracao) -> Result<()> {
        if let Some(dir) = self.caminho.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let linha = a.para_json().escrever();
        let cabem = linha.len() as u64 + 1;
        let teto = self.teto_do_arquivo.load(Ordering::Relaxed);
        let bytes_no_arquivo = std::fs::metadata(&self.caminho)
            .map(|m| m.len())
            .unwrap_or(0);
        let mut arquivo = if crate::rodizio::deve_girar(bytes_no_arquivo, cabem, teto) {
            let manter = self.manter.load(Ordering::Relaxed);
            let (novo, deu_errado) = crate::rodizio::girar(&self.caminho, manter);
            self.rodizios.fetch_add(1, Ordering::Relaxed);
            if deu_errado {
                self.falhas_de_rodizio.fetch_add(1, Ordering::Relaxed);
            }
            match novo {
                Some(f) => f,
                // O `girar` ja tentou reabrir e falhou -- tentar de novo
                // aqui deixa o `?` abaixo contar o erro REAL do sistema de
                // arquivos, em vez de engolir a falha em silencio.
                None => OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.caminho)?,
            }
        } else {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.caminho)?
        };
        writeln!(arquivo, "{linha}")?;
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

    /// Pedido 228, sentido 1: **sem configurar nada, o comportamento e o de
    /// sempre** -- o `diretivas.log` so cresce, sem `.1`. Regra petrea
    /// "guarda nova entra pedida": rodizio que ninguem pediu nao pode mudar
    /// quem nao mexeu no campo novo.
    #[test]
    fn sem_rodizio_configurado_o_diario_so_cresce() {
        let d = DirTemp::novo("diario-sem-rodizio");
        let diario = Diario::ao_lado_de(&d.join("acessos.log"));
        assert_eq!(diario.teto_do_arquivo(), 0, "nasce sem rodizio");
        for n in 1..=30 {
            diario
                .registrar(&alteracao(
                    "max_linhas",
                    Json::Numero(0.0),
                    Json::Numero(n as f64),
                ))
                .unwrap();
        }
        assert_eq!(diario.rodizios(), 0, "teto 0 nunca gira");
        assert!(!crate::rodizio::com_sufixo(diario.caminho(), 1).exists());
        assert_eq!(diario.ultimas(100).len(), 30);
        std::fs::remove_dir_all(&d).unwrap();
    }

    /// Pedido 228, sentido 2: **passar do teto configurado gira o
    /// `diretivas.log`**. Prova real do defeito reposto: com
    /// `crate::rodizio::deve_girar` forcado a `false` (o defeito antigo --
    /// crescer para sempre), este teste falha em `rodizios() > 0` e na
    /// existencia do `.1`; com o rodizio ligado, os dois passam.
    #[test]
    fn estourar_o_teto_gira_o_diario() {
        let d = DirTemp::novo("diario-com-rodizio");
        let diario = Diario::ao_lado_de(&d.join("acessos.log"));
        // Teto minusculo e `manter` folgado -- a mesma cautela do teste
        // irmao em `acesso.rs`: `manter` tem de cobrir tudo o que este
        // teste gera, senao o proprio rodizio, funcionando certo, descarta
        // o mais velho de proposito e o "nada se perde" abaixo falharia por
        // um motivo que nao e o defeito provado aqui.
        diario.definir_rodizio(80, 20);
        for n in 1..=10 {
            diario
                .registrar(&alteracao(
                    "max_linhas",
                    Json::Numero(0.0),
                    Json::Numero(n as f64),
                ))
                .unwrap();
        }
        assert!(
            diario.rodizios() > 0,
            "tinha de ter girado pelo menos uma vez"
        );
        assert_eq!(
            diario.falhas_de_rodizio(),
            0,
            "nenhum passo do rodizio falhou"
        );
        assert!(
            crate::rodizio::com_sufixo(diario.caminho(), 1).exists(),
            "o rodizio tinha de deixar um .1 para tras"
        );
        // Nada se perde: somando o corrente com TODOS os antigos, as 10
        // diretivas continuam legiveis.
        let mut total = diario.ultimas(1000).len();
        for n in 1..=diario.manter() {
            let sufixo = crate::rodizio::com_sufixo(diario.caminho(), n);
            if !sufixo.exists() {
                break;
            }
            let d_antigo = Diario {
                caminho: sufixo,
                teto_do_arquivo: AtomicU64::new(0),
                manter: AtomicUsize::new(0),
                rodizios: AtomicU64::new(0),
                falhas_de_rodizio: AtomicU64::new(0),
            };
            total += d_antigo.ultimas(1000).len();
        }
        assert_eq!(total, 10, "girar nao pode perder nem duplicar diretiva");
        std::fs::remove_dir_all(&d).unwrap();
    }
}
