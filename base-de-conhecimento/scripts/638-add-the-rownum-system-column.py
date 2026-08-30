# Add the rownum system column
# 28/08 18:26

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()

s=s.replace('''pub const COLUNA_SOFTDELETED: &str = "softdeleted";''',
'''pub const COLUNA_SOFTDELETED: &str = "softdeleted";

/// Nome da coluna de sistema com o numero de ordem de chegada da linha.
///
/// # Por que ela existe, se ja ha o rowid
///
/// O `rowid` e a POSICAO FISICA. Enquanto o volume sai de divisao, posicao e
/// ordem de chegada sao a mesma coisa, e o rowid serve de cursor sozinho. Na
/// particao ALFANUMERICA nao sao: a linha vai para o volume da letra dela, e
/// duas linhas digitadas em seguida caem em arquivos diferentes, com rowids
/// que nao se comparam.
///
/// O `rownum` e o que sobra de monotonico: um contador global da tabela,
/// atribuido na insercao, que nunca reaproveita numero. A ordem de digitacao
/// nao se perde na particao alfanumerica -- ela muda de campo.
pub const COLUNA_ROWNUM: &str = "rownum";''',1)

# a coluna entra DEPOIS da softdeleted, para nao deslocar o que ja existe
velho='''        if !colunas.iter().any(|c| c.nome == COLUNA_SOFTDELETED) {
            colunas.push(
                Column::new(COLUNA_SOFTDELETED, ColumnType::Bool)
                    .obrigatoria()
                    .com_caption("Excluido")
                    .com_descricao(
                        "Marca a linha como excluida sem apagar. \\
                         O motivo fica no .reason.",
                    ),
            );
        }
        Schema::do_disco(nome, colunas, indices)'''
novo='''        if !colunas.iter().any(|c| c.nome == COLUNA_SOFTDELETED) {
            colunas.push(
                Column::new(COLUNA_SOFTDELETED, ColumnType::Bool)
                    .obrigatoria()
                    .com_caption("Excluido")
                    .com_descricao(
                        "Marca a linha como excluida sem apagar. \\
                         O motivo fica no .reason.",
                    ),
            );
        }
        // DEPOIS da softdeleted, e nao antes: coluna de sistema nova entra
        // sempre no fim, senao uma tabela gravada na versao anterior teria os
        // offsets deslocados ao ser relida.
        //
        // `UInt8` e nao `Sequence`: uma tabela so pode ter uma coluna
        // `Sequence` -- o contador do `.reg` e unico --, e reservar essa unica
        // vaga para o motor tiraria do usuario um tipo que e dele. O `rownum`
        // tem contador proprio.
        if !colunas.iter().any(|c| c.nome == COLUNA_ROWNUM) {
            colunas.push(
                Column::new(COLUNA_ROWNUM, ColumnType::UInt8)
                    .obrigatoria()
                    .com_caption("Nº")
                    .com_descricao(
                        "Ordem de chegada da linha. O motor preenche; \\
                         nunca reaproveita numero.",
                    ),
            );
        }
        Schema::do_disco(nome, colunas, indices)'''
assert velho in s
s=s.replace(velho,novo,1)

# validacao e acessor
velho2='''        let bitmap_len = colunas.len().div_ceil(8);'''
novo2='''        if let Some(c) = colunas.iter().find(|c| c.nome == COLUNA_ROWNUM) {
            if c.ty != ColumnType::UInt8 {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWNUM} e do motor e tem de ser UInt8; \\
                     esta declarada como {:?}",
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_ROWNUM} nao pode aceitar nulo: \\
                     linha sem numero de ordem nao pagina"
                )));
            }
        }

        let bitmap_len = colunas.len().div_ceil(8);'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

velho3='''    /// Exigir motivo escrito na exclusao. Escolhido ao criar a tabela.'''
novo3='''    /// Posicao da coluna de sistema `rownum`.
    ///
    /// `None` numa tabela gravada antes da v5 do esquema.
    pub fn coluna_rownum(&self) -> Option<usize> {
        self.colunas.iter().position(|c| c.nome == COLUNA_ROWNUM)
    }

    /// Exigir motivo escrito na exclusao. Escolhido ao criar a tabela.'''
assert velho3 in s
s=s.replace(velho3,novo3,1)

# versao do esquema
s=s.replace('''/// A 4 acrescentou a coluna de sistema [`COLUNA_SOFTDELETED`] e o sinal de
/// motivo obrigatorio.''','''/// A 4 acrescentou a coluna de sistema [`COLUNA_SOFTDELETED`] e o sinal de
/// motivo obrigatorio. A 5 acrescentou [`COLUNA_ROWNUM`].''',1)
s=s.replace('const VERSAO_ESQUEMA: u16 = 4;','const VERSAO_ESQUEMA: u16 = 5;',1)
s=s.replace('/// sorteado na hora e os textos vazios. Escrever, so na 4.','/// sorteado na hora e os textos vazios. Escrever, so na 5.',1)
io.open(p,'w',encoding='utf-8').write(s)
