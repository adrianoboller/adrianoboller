//! A ficha COMPARTILHADA: o que uma leitura alcanca, e nada mais.
//!
//! # O invariante, e por que ele nao cabe num comentario
//!
//! A trava de dados do servidor nunca protegeu a [`Instancia`]: ela tem um
//! campo, um `PathBuf` imutavel, e todo metodo dela e `&self`. O que a trava
//! e, de verdade, e uma FICHA DE EXCLUSAO -- quem a tem mexe no disco --, e o
//! estado protegido esta la fora, nos arquivos.
//!
//! Enquanto a ficha era uma so, o invariante era «uma operacao toca os
//! arquivos de cada vez». Com duas fichas ele passa a ser:
//!
//! > **N leitores podem tocar os arquivos ao mesmo tempo, e o que torna isso
//! > seguro e nenhum deles escrever.**
//!
//! «Nenhum deles escrever» e uma convencao -- e convencao que o compilador nao
//! conhece e convencao que uma refacao apaga em silencio. Foi exatamente esse
//! o defeito que o marcador `!Sync` da [`Instancia`] existe para impedir:
//! `RwLock<Instancia>` **compila de primeira e esta errado**, porque `&self` e
//! o que um guard de LEITURA entrega, e dois escritores tomariam guard de
//! leitura e abririam dois `Table` sobre os mesmos arquivos.
//!
//! Este modulo e a resposta: a ficha compartilhada entrega uma
//! [`TabelaLeitura`], que **nao tem** metodo de escrita nenhum. Escrever por
//! ela nao e um erro de disciplina -- e um erro de COMPILACAO, e o
//! `escrever_por_uma_tabela_de_leitura_nao_compila` prova isso nos dois
//! sentidos: o mesmo codigo contra o `Table` compila.
//!
//! [`Instancia`]: crate::catalogo::Instancia

use phxsql_core::error::Result;
use phxsql_core::schema::Schema;
use phxsql_core::RowId;

use crate::table::{Linha, Salto, Sobreposicao, Table, Visao};

/// O que uma VARREDURA precisa de uma tabela -- e so isso.
///
/// # Por que um trait, e nao dois corpos de varredura
///
/// Porque a varredura tem de rodar igual nas duas fichas, e duas copias dela
/// divergiriam: o dia em que a paginacao por indice mudasse de um lado e nao
/// do outro, a mesma consulta responderia coisas diferentes conforme a tabela
/// tivesse ou nao dado pessoal. E a mesma razao pela qual o `casa` do filtro
/// mora no `phxsql-store` e nao em duas casas.
///
/// E ele carrega uma garantia de tabela: **nenhum metodo aqui escreve**. O
/// corpo generico que os chama, portanto, tambem nao escreve -- em qualquer
/// das duas fichas. Acrescentar um metodo de escrita a este trait e o jeito de
/// quebrar a ficha compartilhada, e por isso a lista e curta e explicita em
/// vez de um `Deref` para o `Table` inteiro.
pub trait Legivel {
    fn esquema(&self) -> &Schema;
    fn registros(&self) -> u64;
    fn marcadas(&self) -> u64;
    fn tem_dado_pessoal(&self) -> bool;
    fn ler(&mut self, rowid: RowId) -> Result<Option<Linha>>;
    fn contar(&mut self, visao: Visao) -> Result<u64>;
    fn rownum_de(&mut self, rowid: RowId) -> Result<u64>;
    fn pagina_por_indice(
        &mut self,
        indice: &str,
        visao: Visao,
        pular: u64,
        limite: u64,
    ) -> Result<Vec<RowId>>;
    fn pagina_por_posicao(
        &mut self,
        pular: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<(Vec<RowId>, Salto)>;
    fn pagina_depois_de(&mut self, cursor: RowId, limite: u64, visao: Visao) -> Result<Vec<RowId>>;
    fn pagina_antes_de(&mut self, cursor: RowId, limite: u64, visao: Visao) -> Result<Vec<RowId>>;
    fn pagina_desde_rownum(&mut self, alvo: u64, limite: u64, visao: Visao) -> Result<Vec<RowId>>;
}

/// A tabela aberta com a ficha exclusiva tambem sabe ser lida.
///
/// Sem esta implementacao, o corpo generico da varredura so serviria a ficha
/// compartilhada e a outra metade viraria a segunda copia.
impl Legivel for Table {
    fn esquema(&self) -> &Schema {
        Table::esquema(self)
    }
    fn registros(&self) -> u64 {
        Table::registros(self)
    }
    fn marcadas(&self) -> u64 {
        Table::marcadas(self)
    }
    fn tem_dado_pessoal(&self) -> bool {
        Table::tem_dado_pessoal(self)
    }
    fn ler(&mut self, rowid: RowId) -> Result<Option<Linha>> {
        Table::ler(self, rowid)
    }
    fn contar(&mut self, visao: Visao) -> Result<u64> {
        Table::contar(self, visao)
    }
    fn rownum_de(&mut self, rowid: RowId) -> Result<u64> {
        Table::rownum_de(self, rowid)
    }
    fn pagina_por_indice(
        &mut self,
        indice: &str,
        visao: Visao,
        pular: u64,
        limite: u64,
    ) -> Result<Vec<RowId>> {
        Table::pagina_por_indice(self, indice, visao, pular, limite)
    }
    fn pagina_por_posicao(
        &mut self,
        pular: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<(Vec<RowId>, Salto)> {
        Table::pagina_por_posicao(self, pular, limite, visao)
    }
    fn pagina_depois_de(&mut self, cursor: RowId, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        Table::pagina_depois_de(self, cursor, limite, visao)
    }
    fn pagina_antes_de(&mut self, cursor: RowId, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        Table::pagina_antes_de(self, cursor, limite, visao)
    }
    fn pagina_desde_rownum(&mut self, alvo: u64, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        Table::pagina_desde_rownum(self, alvo, limite, visao)
    }
}

/// Uma tabela que **so** se le -- a unica coisa que a ficha compartilhada
/// entrega.
///
/// # Por que ela envolve o `Table` em vez de derivar dele
///
/// Um `Deref<Target = Table>` seria uma linha e devolveria o `Table` inteiro a
/// quem chama, incluindo `inserir`, `excluir` e `sincronizar`: a garantia
/// morreria no mesmo commit em que nasceu. O envelope custa a lista de metodos
/// do [`Legivel`] e paga com o compilador do lado de quem vier depois.
///
/// # O que ela NAO sabe fazer, e o que isso obriga
///
/// Nao sabe assinar o `.log`, nao sabe registrar acesso na trilha de dado
/// pessoal e nao sabe criar o espelho `.bkp`. As tres sao escritas, e as tres
/// acontecem hoje em caminhos de LEITURA -- por isso quem chama confere
/// [`Legivel::tem_dado_pessoal`] e [`TabelaLeitura::tem_espelho`] e desce para
/// a ficha exclusiva quando alguma delas manda.
///
/// # A prova, e ela e nos DOIS sentidos
///
/// Gravar por uma tabela de leitura nao compila:
///
/// ```compile_fail
/// fn grava(t: &mut phxsql_store::leitura::TabelaLeitura) {
///     let _ = t.inserir(&[]);
/// }
/// ```
///
/// E o CONTROLE, que tem de compilar -- sem ele o de cima passaria tambem por
/// um engano de digitacao, e um teste que passa por engano e pior que um teste
/// que falta:
///
/// ```
/// fn grava(t: &mut phxsql_store::table::Table) {
///     let _ = t.inserir(&[]);
/// }
/// ```
pub struct TabelaLeitura(Table);

impl TabelaLeitura {
    /// Envolve uma tabela ja aberta SEM escrever.
    ///
    /// `pub(crate)` de proposito: quem estiver fora do `phxsql-store` nao pode
    /// escolher qual `Table` vira leitura, senao bastaria abrir uma pela ficha
    /// exclusiva e vesti-la de leitura para a garantia virar enfeite.
    pub(crate) fn nova(t: Table) -> TabelaLeitura {
        TabelaLeitura(t)
    }

    /// A tabela ja tem o espelho `.bkp`?
    ///
    /// Quem chama precisa da resposta ANTES de decidir a ficha: com
    /// `recursos.espelho` ligado, abrir uma tabela sem espelho o CRIA -- e
    /// criar arquivo e escrever.
    pub fn tem_espelho(&self) -> bool {
        self.0.tem_espelho()
    }

    /// O que a transacao desta conexao ja pediu e ainda nao gravou.
    ///
    /// Mora em memoria, nao no disco: e a unica coisa que uma leitura "muda", e
    /// ela muda o que ESTA leitura enxerga, nunca o que esta gravado. Sem ela,
    /// a ficha compartilhada mostraria o disco enquanto a exclusiva mostra a
    /// transacao, e quem olha a tela nao teria como saber qual das duas mente.
    pub fn sobrepor(&mut self, s: Sobreposicao) {
        self.0.sobrepor(s);
    }
}

impl Legivel for TabelaLeitura {
    fn esquema(&self) -> &Schema {
        self.0.esquema()
    }
    fn registros(&self) -> u64 {
        self.0.registros()
    }
    fn marcadas(&self) -> u64 {
        self.0.marcadas()
    }
    fn tem_dado_pessoal(&self) -> bool {
        self.0.tem_dado_pessoal()
    }
    fn ler(&mut self, rowid: RowId) -> Result<Option<Linha>> {
        self.0.ler(rowid)
    }
    fn contar(&mut self, visao: Visao) -> Result<u64> {
        self.0.contar(visao)
    }
    fn rownum_de(&mut self, rowid: RowId) -> Result<u64> {
        self.0.rownum_de(rowid)
    }
    fn pagina_por_indice(
        &mut self,
        indice: &str,
        visao: Visao,
        pular: u64,
        limite: u64,
    ) -> Result<Vec<RowId>> {
        self.0.pagina_por_indice(indice, visao, pular, limite)
    }
    fn pagina_por_posicao(
        &mut self,
        pular: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<(Vec<RowId>, Salto)> {
        self.0.pagina_por_posicao(pular, limite, visao)
    }
    fn pagina_depois_de(&mut self, cursor: RowId, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        self.0.pagina_depois_de(cursor, limite, visao)
    }
    fn pagina_antes_de(&mut self, cursor: RowId, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        self.0.pagina_antes_de(cursor, limite, visao)
    }
    fn pagina_desde_rownum(&mut self, alvo: u64, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        self.0.pagina_desde_rownum(alvo, limite, visao)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;
    use crate::catalogo::{Aberta, Raiz};
    use phxsql_core::schema::{Column, IndexColumn, IndexDef};
    use phxsql_core::types::ColumnType;
    use phxsql_core::value::Value;

    fn dir_temp(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("leitura-{rotulo}"))
    }

    fn esquema() -> Schema {
        Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("nome", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    /// A ficha compartilhada le a mesma pagina que a exclusiva.
    ///
    /// Sem este teste, a garantia de tipo estaria provada e a RESPOSTA nao: um
    /// envelope que devolvesse a pagina errada tambem nao compilaria escrita.
    #[test]
    fn as_duas_fichas_leem_a_mesma_pagina() {
        let d = dir_temp("duas-fichas");
        let mut raiz = Raiz::nova(&d.0).unwrap();
        {
            let inst = raiz.exclusiva();
            let db = inst.garantir_database("b").unwrap();
            let mut t = db.criar_tabela(None, esquema()).unwrap();
            for i in 1..=10u64 {
                t.inserir(&[Value::Int(i as i64), Value::Str(format!("n{i}"))])
                    .unwrap();
            }
            t.sincronizar().unwrap();
        }
        let pela_exclusiva = {
            let inst = raiz.exclusiva();
            let mut t = inst
                .abrir_database("b")
                .unwrap()
                .abrir_qualificada("clientes")
                .unwrap();
            t.pagina_por_posicao(2, 3, Visao::Ativas).unwrap().0
        };
        let Aberta::Pronta(mut t) = raiz.abrir_para_ler("b", "clientes").unwrap() else {
            panic!("uma tabela recem-criada e sincronizada abre sem escrever");
        };
        assert_eq!(
            t.pagina_por_posicao(2, 3, Visao::Ativas).unwrap().0,
            pela_exclusiva,
            "a ficha compartilhada devolveu outra pagina"
        );
        assert_eq!(t.contar(Visao::Ativas).unwrap(), 10);
        assert_eq!(t.registros(), 10);
    }

    /// A tabela sem `.trash` nem `.reason` MANDA para a ficha exclusiva.
    ///
    /// E o defeito reposto de propria mao: apagar os dois arquivos poe a
    /// tabela no estado em que abrir CRIA -- que e o estado de toda tabela
    /// gravada antes de eles existirem. Se `abrir_para_ler` os criasse, dois
    /// leitores simultaneos criariam o mesmo arquivo.
    #[test]
    fn a_tabela_que_precisaria_escrever_para_abrir_manda_para_a_exclusiva() {
        let d = dir_temp("precisa-escrever");
        let mut raiz = Raiz::nova(&d.0).unwrap();
        {
            let inst = raiz.exclusiva();
            let db = inst.garantir_database("b").unwrap();
            let mut t = db.criar_tabela(None, esquema()).unwrap();
            t.inserir(&[Value::Int(1), Value::Str("um".into())])
                .unwrap();
            t.sincronizar().unwrap();
        }
        // O controle: antes de mexer, ela abre pela ficha compartilhada.
        assert!(matches!(
            raiz.abrir_para_ler("b", "clientes").unwrap(),
            Aberta::Pronta(_)
        ));
        std::fs::remove_file(d.0.join("b/clientes.trash")).unwrap();
        match raiz.abrir_para_ler("b", "clientes").unwrap() {
            Aberta::PrecisaDaFichaExclusiva(porque) => {
                assert!(porque.contains("lixeira"), "motivo obscuro: {porque}")
            }
            Aberta::Pronta(_) => panic!("abriu criando a lixeira sob a ficha compartilhada"),
        }
        // E a exclusiva continua abrindo, criando o que falta -- senao o
        // conserto teria trocado uma tabela que abre por uma que nao abre.
        let inst = raiz.exclusiva();
        inst.abrir_database("b")
            .unwrap()
            .abrir_qualificada("clientes")
            .unwrap();
        assert!(d.0.join("b/clientes.trash").exists());
    }
}
