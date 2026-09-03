//! O verificador de consistencia referencial. **Ele RELATA, nao conserta.**
//!
//! # Por que so relata
//!
//! Consertar dado do dono sem ele pedir e pior que o defeito. Uma orfa pode
//! ser lixo de importacao, e pode ser a unica copia de um pedido cujo cliente
//! alguem apagou por engano -- e as duas sao indistinguiveis daqui. Apagar a
//! orfa destroi a segunda; inventar a mae inventa dado. O que este modulo faz
//! e o que so ele pode fazer: **dizer onde esta**, com tabela, chave, rowid e
//! valor, para que a decisao seja de quem tem como tomar.
//!
//! # As tres perguntas que ele faz
//!
//! 1. **A tabela mae existe?** Uma chave que aponta para tabela que nao existe
//!    nao e uma orfa: e um modelo quebrado, e nenhuma linha filha pode ser
//!    gravada enquanto ele estiver assim.
//! 2. **Ha indice dos DOIS lados?** Na mae para responder «existe este pai?»
//!    ao gravar a filha, e na filha para responder «alguem aponta para esta
//!    linha?» ao apagar a mae. Sem um deles o motor RECUSA a gravacao dizendo
//!    qual falta -- entao a falta e um defeito que trava o banco, e nao uma
//!    lentidao. Ela aparece aqui antes de aparecer no dia do primeiro
//!    `excluir`.
//! 3. **Cada linha filha tem mae VIVA?** «Existir» nao e «estar viva»: a mae
//!    excluida de forma suave continua no `.reg` com a chave no indice, e uma
//!    filha apontando para ela e a orfa que ninguem ve.
//!
//! # O que ele varre, e o que isso custa
//!
//! Toda linha de toda tabela que declara chave, viva ou marcada -- a marcada
//! tambem, porque o `conferir_filhas` a conta como filha e ela segura a mae.
//! A mae abre **uma vez por chave**, e nao por linha: abrir custa 46,8 us
//! medidos (`docs/DESEMPENHO.md` §15), e paga-lo por linha faria a varredura
//! de um milhao de linhas custar mais de um minuto so em abertura.
//!
//! ```bash
//! cargo run --release --example conferir-integridade -p phxsql-store -- <diretorio>
//! ```

use std::path::Path;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::ForeignKey;
use phxsql_core::value::Value;

use crate::table::{indice_que_cobre, nome_simples, Table};

/// O que ha de errado com uma chave, ou com uma linha por ela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Falha {
    /// A tabela referenciada nao existe neste diretorio.
    TabelaMaeAusente,
    /// A mae nao tem indice comecando pelas colunas referenciadas.
    SemIndiceNaMae,
    /// A filha nao tem indice comecando pelas colunas da chave.
    SemIndiceNaFilha,
    /// Nao existe linha mae com esse valor.
    MaeAusente,
    /// Existe, mas esta marcada como excluida.
    MaeExcluida,
}

impl Falha {
    pub fn texto(&self) -> &'static str {
        match self {
            Falha::TabelaMaeAusente => "a tabela mae nao existe neste banco",
            Falha::SemIndiceNaMae => "a mae nao tem indice pelas colunas referenciadas",
            Falha::SemIndiceNaFilha => "a filha nao tem indice pelas colunas da chave",
            Falha::MaeAusente => "nao existe linha mae com esse valor",
            Falha::MaeExcluida => "a linha mae existe, mas esta EXCLUIDA",
        }
    }

    /// Uma falha de ESTRUTURA vale para a chave inteira; uma de LINHA vale so
    /// para a linha que a carrega. Separa-las e o que impede o relatorio de
    /// dizer «um milhao de violacoes» quando o que falta e um indice so.
    pub fn e_de_estrutura(&self) -> bool {
        matches!(
            self,
            Falha::TabelaMaeAusente | Falha::SemIndiceNaMae | Falha::SemIndiceNaFilha
        )
    }
}

/// Uma violacao, com onde ela esta.
#[derive(Debug, Clone)]
pub struct Violacao {
    pub tabela: String,
    pub chave: String,
    /// `None` nas falhas de estrutura, que sao da chave e nao de uma linha.
    pub rowid: Option<u64>,
    /// O valor da chave na linha filha, quando ha linha.
    pub valor: Vec<Value>,
    pub falha: Falha,
    /// A chave pediu conferencia (`verificar`)? Chave sem ela pode ter orfa
    /// sem que ninguem tenha quebrado promessa nenhuma -- e o relatorio diz a
    /// diferenca em vez de misturar as duas.
    pub conferida: bool,
}

impl std::fmt::Display for Violacao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.tabela, self.chave)?;
        if let Some(r) = self.rowid {
            write!(f, " rowid {r}")?;
        }
        if !self.valor.is_empty() {
            let v: Vec<String> = self.valor.iter().map(|x| format!("{x:?}")).collect();
            write!(f, " ({})", v.join(", "))?;
        }
        if !self.conferida {
            write!(f, " [nao conferida]")?;
        }
        write!(f, ": {}", self.falha.texto())
    }
}

/// O que a varredura viu, e o que ela conseguiu ver.
#[derive(Debug, Default)]
pub struct Relatorio {
    pub violacoes: Vec<Violacao>,
    /// Tabelas visitadas.
    pub tabelas: usize,
    /// Chaves estrangeiras conferidas (as declaradas, todas).
    pub chaves: usize,
    /// Linhas lidas.
    pub linhas: u64,
    /// Tabelas que nao abriram, com o motivo.
    ///
    /// Nao se mistura com violacao: tabela quebrada e outro defeito, e some-la
    /// no mesmo balde faria o relatorio dizer que ha orfa onde ha arquivo
    /// corrompido.
    pub nao_abriram: Vec<(String, String)>,
}

impl Relatorio {
    pub fn limpo(&self) -> bool {
        self.violacoes.is_empty() && self.nao_abriram.is_empty()
    }
}

/// Confere UMA chave de uma tabela filha ja aberta.
///
/// `parar_na_primeira` existe para quem so precisa saber SE ha violacao -- a
/// declaracao de uma chave conferida, que recusa na primeira. O verificador
/// passa `false` e leva a lista inteira.
///
/// A mae abre **uma vez**, e nao por linha: ver a nota do modulo.
pub fn conferir_chave(
    filha: &mut Table,
    fk: &ForeignKey,
    parar_na_primeira: bool,
) -> Result<Vec<Violacao>> {
    let mut saida = Vec::new();
    let nome_filha = filha.nome().to_string();
    let violacao = |rowid, valor, falha| Violacao {
        tabela: nome_filha.clone(),
        chave: fk.nome.clone(),
        rowid,
        valor,
        falha,
        conferida: fk.verificar,
    };

    // A filha precisa de indice pela chave: e ele que responde «alguem aponta
    // para esta linha?» quando a mae tenta sair.
    let colunas_da_filha: Vec<String> = fk
        .colunas
        .iter()
        .filter_map(|&c| filha.esquema().colunas().get(c).map(|x| x.nome.clone()))
        .collect();
    if colunas_da_filha.len() != fk.colunas.len()
        || indice_que_cobre(filha.esquema(), &colunas_da_filha).is_none()
    {
        saida.push(violacao(None, Vec::new(), Falha::SemIndiceNaFilha));
    }

    let diretorio = filha.diretorio().to_path_buf();
    let mut mae = match Table::abrir(&diretorio, nome_simples(&fk.tabela_ref)) {
        Ok(m) => m,
        Err(PhxError::NaoEncontrado(_)) => {
            saida.push(violacao(None, Vec::new(), Falha::TabelaMaeAusente));
            return Ok(saida);
        }
        Err(e) => return Err(e),
    };
    let Some(indice) = indice_que_cobre(mae.esquema(), &fk.colunas_ref) else {
        saida.push(violacao(None, Vec::new(), Falha::SemIndiceNaMae));
        // Sem indice na mae nao ha como perguntar por linha: o motor procura
        // por indice, nunca por varredura, e inventar aqui uma varredura que a
        // gravacao recusa faria o relatorio medir outra coisa.
        return Ok(saida);
    };

    // Linha a linha, pelo slot: o `.reg` nunca reaproveita slot, entao o
    // intervalo `1..=slots` cobre tudo o que ja existiu, e o `ler` devolve
    // `None` no que ja saiu. Sem montar a tabela inteira em memoria.
    for rowid in 1..=filha.slots() {
        let Some(linha) = filha.ler(rowid)? else {
            continue;
        };
        let mut chave = Vec::with_capacity(fk.colunas.len());
        let mut tem_nulo = false;
        for &c in &fk.colunas {
            match linha.get(c) {
                None | Some(Value::Null) => {
                    tem_nulo = true;
                    break;
                }
                Some(v) => chave.push(v.clone()),
            }
        }
        // NULO satisfaz -- o mesmo `MATCH SIMPLE` da gravacao. Conferir aqui
        // e nao la faria o verificador acusar linha que o motor aceita.
        if tem_nulo {
            continue;
        }
        let achou = mae.buscar(&indice, &chave)?;
        let falha = if achou.is_empty() {
            Some(Falha::MaeAusente)
        } else {
            let viva = achou.iter().try_fold(false, |viva, &r| -> Result<bool> {
                if viva {
                    return Ok(true);
                }
                Ok(match mae.ler(r)? {
                    Some(l) => !mae.esta_excluida(&l),
                    None => false,
                })
            })?;
            if viva {
                None
            } else {
                Some(Falha::MaeExcluida)
            }
        };
        if let Some(f) = falha {
            saida.push(violacao(Some(rowid), chave, f));
            if parar_na_primeira {
                return Ok(saida);
            }
        }
    }
    Ok(saida)
}

/// Varre um diretorio de tabelas e relata tudo o que achar.
pub fn conferir_diretorio(diretorio: &Path) -> Result<Relatorio> {
    let mut r = Relatorio::default();
    for nome in crate::catalogo::tabelas_em(diretorio)? {
        // Tabela que nao abre nao trava a varredura: o defeito dela e dela, e
        // misturar os dois faria uma tabela quebrada esconder as orfas das
        // outras. Mesmo julgamento do `conferir_filhas`.
        let mut t = match Table::abrir(diretorio, &nome) {
            Ok(t) => t,
            Err(e) => {
                r.nao_abriram.push((nome, e.to_string()));
                continue;
            }
        };
        r.tabelas += 1;
        let fks = t.esquema().chaves_estrangeiras().to_vec();
        if fks.is_empty() {
            continue;
        }
        // As linhas contam UMA vez por tabela, e nao uma por chave: duas
        // chaves na mesma tabela leem as mesmas linhas, e somar duas vezes
        // faria o relatorio inflar o proprio trabalho.
        r.linhas += t.registros();
        for fk in &fks {
            r.chaves += 1;
            r.violacoes.extend(conferir_chave(&mut t, fk, false)?);
        }
    }
    Ok(r)
}
