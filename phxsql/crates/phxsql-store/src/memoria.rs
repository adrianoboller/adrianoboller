//! Tabela residente em memoria, e a consulta que roda em cima dela.
//!
//! # Por que existe
//!
//! O `.reg` e rapido porque enderecar um registro e aritmetica -- mas ainda e
//! um `seek` e um `read` no disco. Ha tabela que nao muda quase nunca e e lida
//! o tempo todo: tabela de precos, de cidades, de parametros. Para essas, o
//! disco e o unico custo que sobrou, e da para tira-lo do caminho.
//!
//! Carregada, a tabela vira um vetor em RAM e a consulta nao toca em arquivo
//! nenhum. E o modelo de um Redis(R): o dado mora na memoria, e o disco e o
//! lugar de onde ele veio, nao o lugar por onde ele passa a cada leitura.
//!
//! # O que NAO e
//!
//! Nao e cache que adivinha. Voce carrega quando quer e libera quando quer --
//! nada entra em memoria sozinho. Isso e de proposito: um cache que decide
//! sozinho o que guardar e um cache que um dia decide errado no pior momento.
//!
//! Nao e uma segunda copia que vive a parte. Toda escrita que passa pelo
//! motor atualiza a copia residente no mesmo passo, dentro da mesma trava.
//! Nao existe janela em que o disco e a memoria discordem.
//!
//! # O endereco continua sendo aritmetica
//!
//! As linhas ficam num `Vec` indexado por `rowid - 1`, nao num mapa. O rowid e
//! denso e comeca em 1 -- e o que a ordem de digitacao garante --, entao o
//! acesso e um deslocamento, nao um hash. Slot excluido vira `None` e o rowid
//! seguinte continua de onde estava: a ordem de digitacao vale na memoria
//! exatamente como vale no arquivo.

use std::collections::HashMap;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::paralelo::mapear_faixa;
use phxsql_core::schema::Schema;
use phxsql_core::value::Value;
use phxsql_core::RowId;

use crate::table::{Linha, Table};

/// Como um filtro compara.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operador {
    Igual,
    Diferente,
    Menor,
    MenorIgual,
    Maior,
    MaiorIgual,
    /// Subcadeia, sem distinguir caixa.
    Contem,
    Comeca,
    Termina,
    ENulo,
    NaoENulo,
}

impl Operador {
    pub fn de_texto(s: &str) -> Result<Operador> {
        Ok(match s.trim() {
            "=" | "==" | "igual" => Operador::Igual,
            "!=" | "<>" | "diferente" => Operador::Diferente,
            "<" | "menor" => Operador::Menor,
            "<=" | "menor_igual" => Operador::MenorIgual,
            ">" | "maior" => Operador::Maior,
            ">=" | "maior_igual" => Operador::MaiorIgual,
            "contem" | "like" => Operador::Contem,
            "comeca" => Operador::Comeca,
            "termina" => Operador::Termina,
            "nulo" | "e_nulo" => Operador::ENulo,
            "nao_nulo" | "nao_e_nulo" => Operador::NaoENulo,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "operador desconhecido: {outro:?} (use =, !=, <, <=, >, >=, contem, comeca, termina, nulo, nao_nulo)"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Operador::Igual => "=",
            Operador::Diferente => "!=",
            Operador::Menor => "<",
            Operador::MenorIgual => "<=",
            Operador::Maior => ">",
            Operador::MaiorIgual => ">=",
            Operador::Contem => "contem",
            Operador::Comeca => "comeca",
            Operador::Termina => "termina",
            Operador::ENulo => "nulo",
            Operador::NaoENulo => "nao_nulo",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Filtro {
    pub coluna: usize,
    pub op: Operador,
    pub valor: Value,
}

#[derive(Debug, Clone, Copy)]
pub struct Ordem {
    pub coluna: usize,
    pub desc: bool,
}

/// Uma consulta em memoria. Todos os filtros valem juntos (E, nunca OU) --
/// o OU entra quando alguem precisar, e nao antes.
#[derive(Debug, Clone, Default)]
pub struct Consulta {
    pub onde: Vec<Filtro>,
    pub ordenar: Vec<Ordem>,
    /// Colunas a devolver. Vazio = todas.
    pub colunas: Vec<usize>,
    pub pular: u64,
    /// Zero = sem teto.
    pub max: u64,
}

#[derive(Debug)]
pub struct Resultado {
    pub linhas: Vec<(RowId, Linha)>,
    /// Quantas linhas o motor precisou olhar para responder.
    pub examinadas: u64,
    /// Quantas passaram pelos filtros, antes de `pular` e `max`.
    pub achadas: u64,
    /// Nome da coluna cujo mapa de igualdade evitou a varredura, se houve.
    pub por_mapa: Option<String>,
}

/// Chave canonica de um valor, para o mapa de igualdade.
///
/// Os inteiros de todos os tipos caem no mesmo saco: `Int(5)`, `UInt(5)` e
/// `Date(5)` tem a mesma chave, porque comparam iguais. Se comparassem iguais
/// e tivessem chaves diferentes, o mapa daria uma resposta e a varredura daria
/// outra -- e o defeito so apareceria na tabela grande.
fn chave(v: &Value) -> Vec<u8> {
    let mut k = Vec::with_capacity(17);
    match v {
        Value::Null => k.push(0),
        Value::Bool(b) => {
            k.push(1);
            k.push(u8::from(*b));
        }
        Value::Int(_) | Value::UInt(_) | Value::Date(_) | Value::Time(_) | Value::DateTime(_) => {
            k.push(2);
            k.extend_from_slice(&inteiro_de(v).unwrap_or(0).to_be_bytes());
        }
        Value::Decimal(d) => {
            k.push(2);
            k.extend_from_slice(&d.to_be_bytes());
        }
        Value::Real(r) => {
            k.push(3);
            k.extend_from_slice(&ordenavel(*r).to_be_bytes());
        }
        Value::Str(s) | Value::Memo(s) => {
            k.push(4);
            k.extend_from_slice(s.as_bytes());
        }
        Value::Bin(b) => {
            k.push(5);
            k.extend_from_slice(b);
        }
        // Familia propria: um UUID nunca deve comparar igual a um texto ou a
        // um binario que por acaso tenha os mesmos bytes.
        Value::Uuid(u) => {
            k.push(6);
            k.extend_from_slice(u.bytes());
        }
        Value::Uuid256(u) => {
            k.push(7);
            k.extend_from_slice(u.bytes());
        }
    }
    k
}

fn inteiro_de(v: &Value) -> Option<i128> {
    Some(match v {
        Value::Int(n) => *n as i128,
        Value::UInt(n) => *n as i128,
        Value::Date(n) | Value::Time(n) => *n as i128,
        Value::DateTime(n) => *n as i128,
        Value::Decimal(n) => *n,
        Value::Bool(b) => i128::from(*b),
        _ => return None,
    })
}

/// Bits de um `f64` rearranjados para que a ordem numerica vire ordem de
/// inteiro sem sinal. E o mesmo truque do `keyenc`, pelo mesmo motivo.
fn ordenavel(r: f64) -> u64 {
    let bits = r.to_bits();
    if bits & (1 << 63) != 0 {
        !bits
    } else {
        bits | (1 << 63)
    }
}

/// Ordem total entre dois valores.
///
/// Nulo vem antes de tudo. Numero compara com numero, texto com texto; tipos
/// de familias diferentes nunca sao iguais, e desempatam por familia para a
/// ordenacao ficar estavel em vez de arbitraria.
pub fn comparar(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Real(_), _) | (_, Value::Real(_)) => match (numero_real(a), numero_real(b)) {
            (Some(x), Some(y)) => ordenavel(x).cmp(&ordenavel(y)),
            _ => familia(a).cmp(&familia(b)),
        },
        _ => match (inteiro_de(a), inteiro_de(b)) {
            (Some(x), Some(y)) => x.cmp(&y),
            _ => match (texto_de(a), texto_de(b)) {
                (Some(x), Some(y)) => x.cmp(y),
                _ => match (a, b) {
                    (Value::Bin(x), Value::Bin(y)) => x.cmp(y),
                    // Big-endian: comparar bytes e comparar o numero. Num v7
                    // isso quer dizer comparar o instante de criacao.
                    (Value::Uuid(x), Value::Uuid(y)) => x.cmp(y),
                    (Value::Uuid256(x), Value::Uuid256(y)) => x.cmp(y),
                    _ => familia(a).cmp(&familia(b)),
                },
            },
        },
    }
}

fn numero_real(v: &Value) -> Option<f64> {
    match v {
        Value::Real(r) => Some(*r),
        outro => inteiro_de(outro).map(|n| n as f64),
    }
}

fn texto_de(v: &Value) -> Option<&str> {
    match v {
        Value::Str(s) | Value::Memo(s) => Some(s.as_str()),
        _ => None,
    }
}

fn familia(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Int(_)
        | Value::UInt(_)
        | Value::Decimal(_)
        | Value::Date(_)
        | Value::Time(_)
        | Value::DateTime(_) => 2,
        Value::Real(_) => 3,
        Value::Str(_) | Value::Memo(_) => 4,
        Value::Bin(_) => 5,
        Value::Uuid(_) => 6,
        Value::Uuid256(_) => 7,
    }
}

/// Uma tabela inteira em RAM.
#[derive(Debug)]
pub struct TabelaMemoria {
    esquema: Schema,
    /// Indexado por `rowid - 1`. `None` = slot excluido.
    linhas: Vec<Option<Linha>>,
    /// Mapa de igualdade por coluna, montado no carregamento.
    mapas: HashMap<usize, HashMap<Vec<u8>, Vec<RowId>>>,
    vivos: u64,
    carregada_ms: i64,
    bytes: usize,
}

impl TabelaMemoria {
    /// Le a tabela inteira do disco e monta a copia residente.
    ///
    /// `mapear` sao as colunas que ganham mapa de igualdade. Custa memoria e
    /// paga em consulta: sem mapa, filtrar por igualdade e varredura.
    pub fn carregar(t: &mut Table, mapear: &[usize], agora_ms: i64) -> Result<TabelaMemoria> {
        let esquema = t.esquema().clone();
        let total_colunas = esquema.colunas().len();
        for c in mapear {
            if *c >= total_colunas {
                return Err(PhxError::Esquema(format!(
                    "coluna {c} nao existe em {} (tem {total_colunas})",
                    esquema.nome()
                )));
            }
        }

        let lidas = t.varrer()?;
        let maior = lidas.last().map(|(r, _)| *r).unwrap_or(0);
        let mut linhas: Vec<Option<Linha>> = vec![None; maior as usize];
        let mut bytes = 0usize;
        for (rowid, linha) in lidas {
            bytes += tamanho_da_linha(&linha);
            linhas[(rowid - 1) as usize] = Some(linha);
        }
        let vivos = linhas.iter().filter(|l| l.is_some()).count() as u64;

        let mut mapas = HashMap::new();
        for c in mapear {
            let mut m: HashMap<Vec<u8>, Vec<RowId>> = HashMap::new();
            for (i, linha) in linhas.iter().enumerate() {
                if let Some(l) = linha {
                    m.entry(chave(&l[*c])).or_default().push(i as RowId + 1);
                }
            }
            mapas.insert(*c, m);
        }

        Ok(TabelaMemoria {
            esquema,
            linhas,
            mapas,
            vivos,
            carregada_ms: agora_ms,
            bytes,
        })
    }

    pub fn esquema(&self) -> &Schema {
        &self.esquema
    }

    /// Linhas vivas. Slots excluidos nao contam.
    pub fn vivos(&self) -> u64 {
        self.vivos
    }

    /// Slots, inclusive os excluidos. E o maior rowid ja usado.
    pub fn slots(&self) -> u64 {
        self.linhas.len() as u64
    }

    pub fn carregada_ms(&self) -> i64 {
        self.carregada_ms
    }

    /// Soma do tamanho dos VALORES guardados.
    ///
    /// Nao e quanto o processo cresceu: nao conta o custo por `Vec`, nem a
    /// folga que o alocador reserva, nem os mapas. E o tamanho do dado, que e
    /// o numero comparavel entre uma tabela e outra.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn colunas_mapeadas(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.mapas.keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn ler(&self, rowid: RowId) -> Option<&Linha> {
        if rowid == 0 {
            return None;
        }
        self.linhas.get((rowid - 1) as usize)?.as_ref()
    }

    // --------------------------------------------------- coerencia com o disco

    /// Acompanha uma insercao que ja foi ao disco.
    pub fn anotar_insercao(&mut self, rowid: RowId, linha: &Linha) {
        let i = (rowid - 1) as usize;
        if self.linhas.len() <= i {
            self.linhas.resize(i + 1, None);
        }
        self.tirar_dos_mapas(rowid);
        for (c, m) in self.mapas.iter_mut() {
            m.entry(chave(&linha[*c])).or_default().push(rowid);
        }
        if self.linhas[i].is_none() {
            self.vivos += 1;
        } else if let Some(velha) = &self.linhas[i] {
            self.bytes -= tamanho_da_linha(velha);
        }
        self.bytes += tamanho_da_linha(linha);
        self.linhas[i] = Some(linha.clone());
    }

    /// Acompanha uma alteracao que ja foi ao disco.
    pub fn anotar_alteracao(&mut self, rowid: RowId, linha: &Linha) {
        self.anotar_insercao(rowid, linha);
    }

    /// Acompanha uma exclusao que ja foi ao disco.
    ///
    /// O slot vira `None` e NAO some do vetor: a ordem de digitacao e os
    /// rowids seguintes continuam onde estavam, como no `.reg`.
    pub fn anotar_exclusao(&mut self, rowid: RowId) {
        if rowid == 0 {
            return;
        }
        let i = (rowid - 1) as usize;
        self.tirar_dos_mapas(rowid);
        if let Some(Some(velha)) = self.linhas.get(i) {
            self.bytes -= tamanho_da_linha(velha);
            self.vivos -= 1;
        }
        if let Some(slot) = self.linhas.get_mut(i) {
            *slot = None;
        }
    }

    fn tirar_dos_mapas(&mut self, rowid: RowId) {
        let i = (rowid - 1) as usize;
        let Some(Some(velha)) = self.linhas.get(i) else {
            return;
        };
        for (c, m) in self.mapas.iter_mut() {
            if let Some(lista) = m.get_mut(&chave(&velha[*c])) {
                lista.retain(|r| *r != rowid);
                if lista.is_empty() {
                    m.remove(&chave(&velha[*c]));
                }
            }
        }
    }

    // ------------------------------------------------------------- a consulta

    /// A consulta em memoria. Nenhum acesso a disco acontece aqui.
    pub fn selecionar(&self, c: &Consulta) -> Result<Resultado> {
        let total = self.esquema.colunas().len();
        for f in &c.onde {
            if f.coluna >= total {
                return Err(PhxError::Esquema(format!("coluna {} nao existe", f.coluna)));
            }
        }
        for o in &c.ordenar {
            if o.coluna >= total {
                return Err(PhxError::Esquema(format!("coluna {} nao existe", o.coluna)));
            }
        }
        for p in &c.colunas {
            if *p >= total {
                return Err(PhxError::Esquema(format!("coluna {p} nao existe")));
            }
        }

        // Um filtro de igualdade numa coluna com mapa evita a varredura
        // inteira. Escolhe o primeiro que servir -- e quase sempre o unico.
        let atalho = c.onde.iter().enumerate().find_map(|(i, f)| {
            if f.op != Operador::Igual {
                return None;
            }
            let m = self.mapas.get(&f.coluna)?;
            Some((
                i,
                f.coluna,
                m.get(&chave(&f.valor)).cloned().unwrap_or_default(),
            ))
        });

        let mut examinadas = 0u64;
        let mut passaram: Vec<RowId> = Vec::new();

        match &atalho {
            Some((pulo, _, candidatos)) => {
                for rowid in candidatos {
                    let Some(linha) = self.ler(*rowid) else {
                        continue;
                    };
                    examinadas += 1;
                    if c.onde
                        .iter()
                        .enumerate()
                        .all(|(i, f)| i == *pulo || casa(&linha[f.coluna], f))
                    {
                        passaram.push(*rowid);
                    }
                }
            }
            None => {
                // A varredura sem atalho e o unico trecho desta tabela que
                // divide bem entre nucleos: cada linha e uma pergunta que nao
                // depende das outras, tudo esta em RAM e nada e gravado.
                //
                // `mapear_faixa` preserva a ordem, entao o resultado e o mesmo
                // do laco simples -- uma consulta que mudasse de ordem
                // conforme a maquina seria pior do que uma consulta lenta.
                passaram = mapear_faixa(self.linhas.len(), |i, saida| {
                    let Some(linha) = &self.linhas[i] else { return };
                    if c.onde.iter().all(|f| casa(&linha[f.coluna], f)) {
                        saida.push(i as RowId + 1);
                    }
                });
                // Contar as vivas dentro das threads exigiria um contador
                // compartilhado para nada: o numero e o mesmo, e sai de uma
                // passada barata.
                examinadas = self.linhas.iter().filter(|s| s.is_some()).count() as u64;
            }
        }

        let achadas = passaram.len() as u64;

        if !c.ordenar.is_empty() {
            passaram.sort_by(|a, b| {
                for o in &c.ordenar {
                    let va = &self.ler(*a).expect("rowid vivo")[o.coluna];
                    let vb = &self.ler(*b).expect("rowid vivo")[o.coluna];
                    let ord = comparar(va, vb);
                    let ord = if o.desc { ord.reverse() } else { ord };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                // Empate desempata pela ordem de digitacao, nunca ao acaso.
                a.cmp(b)
            });
        }

        let mut linhas = Vec::new();
        for rowid in passaram.into_iter().skip(c.pular as usize) {
            if c.max > 0 && linhas.len() as u64 >= c.max {
                break;
            }
            let linha = self.ler(rowid).expect("rowid vivo");
            let projetada = if c.colunas.is_empty() {
                linha.clone()
            } else {
                c.colunas.iter().map(|p| linha[*p].clone()).collect()
            };
            linhas.push((rowid, projetada));
        }

        Ok(Resultado {
            linhas,
            examinadas,
            achadas,
            por_mapa: atalho.map(|(_, c, _)| self.esquema.colunas()[c].nome.clone()),
        })
    }
}

fn casa(v: &Value, f: &Filtro) -> bool {
    use std::cmp::Ordering::*;
    match f.op {
        Operador::ENulo => v.e_null(),
        Operador::NaoENulo => !v.e_null(),
        // Nulo nao casa com comparacao nenhuma -- nem com "diferente".
        // Nao se sabe o que ele e, entao nao se afirma nada sobre ele.
        _ if v.e_null() || f.valor.e_null() => false,
        Operador::Igual => comparar(v, &f.valor) == Equal,
        Operador::Diferente => comparar(v, &f.valor) != Equal,
        Operador::Menor => comparar(v, &f.valor) == Less,
        Operador::MenorIgual => matches!(comparar(v, &f.valor), Less | Equal),
        Operador::Maior => comparar(v, &f.valor) == Greater,
        Operador::MaiorIgual => matches!(comparar(v, &f.valor), Greater | Equal),
        Operador::Contem | Operador::Comeca | Operador::Termina => {
            let (Some(t), Some(a)) = (texto_de(v), texto_de(&f.valor)) else {
                return false;
            };
            let t = t.to_lowercase();
            let a = a.to_lowercase();
            match f.op {
                Operador::Contem => t.contains(&a),
                Operador::Comeca => t.starts_with(&a),
                _ => t.ends_with(&a),
            }
        }
    }
}

fn tamanho_da_linha(l: &Linha) -> usize {
    l.iter()
        .map(|v| match v {
            Value::Str(s) | Value::Memo(s) => s.len(),
            Value::Bin(b) => b.len(),
            Value::Decimal(_) => 16,
            Value::Null => 0,
            Value::Bool(_) => 1,
            _ => 8,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;

    fn tabela(dir: &std::path::Path) -> Table {
        let esquema = Schema::new(
            "precos",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("produto", ColumnType::Str(40)).obrigatoria(),
                Column::new("cidade", ColumnType::Str(20)),
                Column::new(
                    "valor",
                    ColumnType::Decimal {
                        precisao: 15,
                        escala: 2,
                    },
                ),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut t = Table::criar(dir, esquema).unwrap();
        let dados = [
            (1, "Cafe", "Blumenau", 1850i128),
            (2, "Acucar", "Joinville", 420),
            (3, "Cafe", "Joinville", 1790),
            (4, "Leite", "Blumenau", 650),
            (5, "Cafe", "Itajai", 1990),
        ];
        for (id, prod, cid, val) in dados {
            t.inserir(&[
                Value::Int(id),
                Value::Str(prod.into()),
                Value::Str(cid.into()),
                Value::Decimal(val),
            ])
            .unwrap();
        }
        t
    }

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn temp(nome: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("mem-{nome}"))
    }

    #[test]
    fn carrega_a_tabela_inteira() {
        let d = temp("carrega");
        let mut t = tabela(&d);
        let m = TabelaMemoria::carregar(&mut t, &[], 1_000).unwrap();
        assert_eq!(m.vivos(), 5);
        assert_eq!(m.slots(), 5);
        assert!(m.bytes() > 0);
        assert_eq!(m.ler(1).unwrap()[1], Value::Str("Cafe".into()));
        assert!(m.ler(6).is_none());
        assert!(m.ler(0).is_none());
    }

    #[test]
    fn filtro_de_igualdade_com_e_sem_mapa_da_o_mesmo() {
        let d = temp("igual");
        let mut t = tabela(&d);
        let sem = TabelaMemoria::carregar(&mut t, &[], 0).unwrap();
        let com = TabelaMemoria::carregar(&mut t, &[1], 0).unwrap();
        let c = Consulta {
            onde: vec![Filtro {
                coluna: 1,
                op: Operador::Igual,
                valor: Value::Str("Cafe".into()),
            }],
            ..Default::default()
        };
        let a = sem.selecionar(&c).unwrap();
        let b = com.selecionar(&c).unwrap();
        assert_eq!(a.achadas, 3);
        assert_eq!(b.achadas, 3);
        assert_eq!(
            a.linhas.iter().map(|(r, _)| *r).collect::<Vec<_>>(),
            b.linhas.iter().map(|(r, _)| *r).collect::<Vec<_>>()
        );
        // O mapa e o ponto: 3 linhas olhadas em vez das 5.
        assert_eq!(a.examinadas, 5);
        assert_eq!(b.examinadas, 3);
        assert_eq!(b.por_mapa.as_deref(), Some("produto"));
        assert!(a.por_mapa.is_none());
    }

    #[test]
    fn a_ordem_de_digitacao_e_o_padrao() {
        let d = temp("ordem");
        let mut t = tabela(&d);
        let m = TabelaMemoria::carregar(&mut t, &[], 0).unwrap();
        let r = m.selecionar(&Consulta::default()).unwrap();
        assert_eq!(
            r.linhas.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn ordena_e_desempata_pela_digitacao() {
        let d = temp("ordena");
        let mut t = tabela(&d);
        let m = TabelaMemoria::carregar(&mut t, &[], 0).unwrap();
        let r = m
            .selecionar(&Consulta {
                ordenar: vec![Ordem {
                    coluna: 1,
                    desc: false,
                }],
                ..Default::default()
            })
            .unwrap();
        let nomes: Vec<&str> = r
            .linhas
            .iter()
            .map(|(_, l)| match &l[1] {
                Value::Str(s) => s.as_str(),
                _ => "?",
            })
            .collect();
        assert_eq!(nomes, vec!["Acucar", "Cafe", "Cafe", "Cafe", "Leite"]);
        // Os tres Cafe saem 1, 3, 5 -- a ordem em que foram digitados.
        let cafes: Vec<RowId> = r
            .linhas
            .iter()
            .filter(|(_, l)| l[1] == Value::Str("Cafe".into()))
            .map(|(r, _)| *r)
            .collect();
        assert_eq!(cafes, vec![1, 3, 5]);
    }

    #[test]
    fn comparacoes_de_faixa_e_texto() {
        let d = temp("faixa");
        let mut t = tabela(&d);
        let m = TabelaMemoria::carregar(&mut t, &[], 0).unwrap();

        let caros = m
            .selecionar(&Consulta {
                onde: vec![Filtro {
                    coluna: 3,
                    op: Operador::MaiorIgual,
                    valor: Value::Decimal(1800),
                }],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(caros.achadas, 2);

        let contem = m
            .selecionar(&Consulta {
                onde: vec![Filtro {
                    coluna: 2,
                    op: Operador::Contem,
                    valor: Value::Str("VILLE".into()),
                }],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(contem.achadas, 2, "contem nao distingue caixa");
    }

    #[test]
    fn nulo_nao_casa_com_comparacao_nenhuma() {
        let v = Value::Null;
        for op in [
            Operador::Igual,
            Operador::Diferente,
            Operador::Menor,
            Operador::Maior,
            Operador::Contem,
        ] {
            assert!(
                !casa(
                    &v,
                    &Filtro {
                        coluna: 0,
                        op,
                        valor: Value::Int(1)
                    }
                ),
                "nulo nao afirma nada com {}",
                op.nome()
            );
        }
        assert!(casa(
            &v,
            &Filtro {
                coluna: 0,
                op: Operador::ENulo,
                valor: Value::Null
            }
        ));
    }

    #[test]
    fn a_escrita_mantem_memoria_e_disco_de_acordo() {
        let d = temp("coerencia");
        let mut t = tabela(&d);
        let mut m = TabelaMemoria::carregar(&mut t, &[1], 0).unwrap();

        let nova = vec![
            Value::Int(6),
            Value::Str("Cafe".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(2100),
            Value::Bool(false),
            Value::UInt(6),
        ];
        let rowid = t.inserir(&nova).unwrap();
        m.anotar_insercao(rowid, &nova);
        assert_eq!(m.vivos(), 6);

        let c = Consulta {
            onde: vec![Filtro {
                coluna: 1,
                op: Operador::Igual,
                valor: Value::Str("Cafe".into()),
            }],
            ..Default::default()
        };
        assert_eq!(m.selecionar(&c).unwrap().achadas, 4, "o mapa viu a nova");

        // Alterar tira do balde velho e poe no novo.
        let trocada = vec![
            Value::Int(6),
            Value::Str("Cha".into()),
            Value::Str("Curitiba".into()),
            Value::Decimal(2100),
            Value::Bool(false),
            Value::UInt(6),
        ];
        t.atualizar(rowid, &trocada).unwrap();
        m.anotar_alteracao(rowid, &trocada);
        assert_eq!(m.selecionar(&c).unwrap().achadas, 3, "saiu do balde Cafe");

        // Excluir vira buraco, e o rowid nao volta.
        t.excluir(1).unwrap();
        m.anotar_exclusao(1);
        assert_eq!(m.vivos(), 5);
        assert_eq!(m.slots(), 6, "o slot fica; a ordem de digitacao nao muda");
        assert!(m.ler(1).is_none());
        assert_eq!(m.selecionar(&c).unwrap().achadas, 2);

        // E o disco concorda com a memoria, linha por linha.
        let do_disco = t.varrer().unwrap();
        assert_eq!(do_disco.len() as u64, m.vivos());
        for (rowid, linha) in do_disco {
            assert_eq!(m.ler(rowid), Some(&linha), "rowid {rowid} divergiu");
        }
    }

    #[test]
    fn projecao_pular_e_teto() {
        let d = temp("projecao");
        let mut t = tabela(&d);
        let m = TabelaMemoria::carregar(&mut t, &[], 0).unwrap();
        let r = m
            .selecionar(&Consulta {
                colunas: vec![1, 3],
                pular: 1,
                max: 2,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(r.achadas, 5, "achadas conta antes de pular e do teto");
        assert_eq!(r.linhas.len(), 2);
        assert_eq!(r.linhas[0].0, 2);
        assert_eq!(r.linhas[0].1.len(), 2, "so as duas colunas pedidas");
    }

    #[test]
    fn coluna_que_nao_existe_e_recusada() {
        let d = temp("coluna");
        let mut t = tabela(&d);
        let m = TabelaMemoria::carregar(&mut t, &[], 0).unwrap();
        for c in [
            Consulta {
                onde: vec![Filtro {
                    coluna: 9,
                    op: Operador::Igual,
                    valor: Value::Int(1),
                }],
                ..Default::default()
            },
            Consulta {
                ordenar: vec![Ordem {
                    coluna: 9,
                    desc: false,
                }],
                ..Default::default()
            },
            Consulta {
                colunas: vec![9],
                ..Default::default()
            },
        ] {
            assert!(m.selecionar(&c).is_err());
        }
        assert!(TabelaMemoria::carregar(&mut t, &[9], 0).is_err());
    }

    #[test]
    fn inteiros_de_tipos_diferentes_comparam_iguais_e_tem_a_mesma_chave() {
        // Se o mapa e a varredura discordassem aqui, o defeito so apareceria
        // na tabela grande -- que e onde ninguem quer descobrir.
        assert_eq!(
            comparar(&Value::Int(5), &Value::UInt(5)),
            std::cmp::Ordering::Equal
        );
        assert_eq!(chave(&Value::Int(5)), chave(&Value::UInt(5)));
        assert_eq!(chave(&Value::Int(5)), chave(&Value::Date(5)));
        assert_ne!(chave(&Value::Int(5)), chave(&Value::Str("5".into())));
        assert_eq!(
            chave(&Value::Str("a".into())),
            chave(&Value::Memo("a".into()))
        );
    }

    #[test]
    fn operador_vem_de_texto() {
        assert_eq!(Operador::de_texto("=").unwrap(), Operador::Igual);
        assert_eq!(Operador::de_texto("<>").unwrap(), Operador::Diferente);
        assert_eq!(Operador::de_texto(" >= ").unwrap(), Operador::MaiorIgual);
        assert_eq!(Operador::de_texto("contem").unwrap(), Operador::Contem);
        assert!(Operador::de_texto("banana").is_err());
    }
}
