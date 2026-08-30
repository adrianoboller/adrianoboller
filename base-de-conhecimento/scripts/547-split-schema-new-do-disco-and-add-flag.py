# Split Schema::new / do_disco and add flag
# 28/08 17:23

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()

# campo novo na struct
velho = '''    paginacao: Paginacao,
    offsets: Vec<usize>,
    bitmap_len: usize,
    payload_len: usize,
}

impl Schema {
    pub fn new(
        nome: impl Into<String>,
        colunas: Vec<Column>,
        indices: Vec<IndexDef>,
    ) -> Result<Schema> {
        let nome = nome.into();'''
novo = '''    paginacao: Paginacao,
    offsets: Vec<usize>,
    bitmap_len: usize,
    payload_len: usize,
    /// Exigir motivo escrito para marcar uma linha como excluida.
    motivo_obrigatorio: bool,
}

impl Schema {
    /// Esquema de uma tabela NOVA.
    ///
    /// Acrescenta a coluna de sistema [`COLUNA_SOFTDELETED`] no fim, se quem
    /// chamou nao a declarou. Quem le esquema do disco nao passa por aqui --
    /// ver [`Schema::do_disco`].
    pub fn new(
        nome: impl Into<String>,
        mut colunas: Vec<Column>,
        indices: Vec<IndexDef>,
    ) -> Result<Schema> {
        if !colunas.iter().any(|c| c.nome == COLUNA_SOFTDELETED) {
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
        Schema::do_disco(nome, colunas, indices)
    }

    /// Esquema montado EXATAMENTE com as colunas dadas, sem acrescentar nada.
    ///
    /// E o caminho da leitura do disco. Acrescentar uma coluna aqui deslocaria
    /// o offset de todas as colunas seguintes e faria o motor ler o campo
    /// errado de cada linha ja gravada -- silenciosamente, porque o CRC do
    /// slot continuaria batendo: os bytes nao mudaram, so a interpretacao.
    pub fn do_disco(
        nome: impl Into<String>,
        colunas: Vec<Column>,
        indices: Vec<IndexDef>,
    ) -> Result<Schema> {
        let nome = nome.into();'''
assert velho in s
s = s.replace(velho, novo, 1)

# validacao da coluna de sistema + campo na construcao final
velho2 = '''        let bitmap_len = colunas.len().div_ceil(8);'''
novo2 = '''        // A coluna de sistema pode ser declarada a mao -- por quem esta
        // recriando uma tabela, por exemplo --, mas nao com outro tipo. Um
        // `softdeleted` Str seria uma coluna comum com nome reservado, e o
        // motor passaria a marcar exclusao num campo que o usuario le como
        // texto.
        if let Some(c) = colunas.iter().find(|c| c.nome == COLUNA_SOFTDELETED) {
            if c.ty != ColumnType::Bool {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_SOFTDELETED} e do motor e tem de ser Bool; \\
                     esta declarada como {:?}",
                    c.ty
                )));
            }
            if c.nullable {
                return Err(PhxError::Esquema(format!(
                    "a coluna {COLUNA_SOFTDELETED} nao pode aceitar nulo: \\
                     nulo seria um terceiro estado entre excluida e nao excluida"
                )));
            }
        }

        let bitmap_len = colunas.len().div_ceil(8);'''
assert velho2 in s
s = s.replace(velho2, novo2, 1)

velho3 = '''            offsets,
            bitmap_len,
            payload_len: pos,
        })
    }'''
novo3 = '''            offsets,
            bitmap_len,
            payload_len: pos,
            motivo_obrigatorio: false,
        })
    }

    /// Posicao da coluna de sistema `softdeleted`.
    ///
    /// `None` numa tabela gravada antes da v4 do esquema: ela nao tem a
    /// coluna, e exclusao suave nela e recusada com essa explicacao.
    pub fn coluna_softdeleted(&self) -> Option<usize> {
        self.colunas.iter().position(|c| c.nome == COLUNA_SOFTDELETED)
    }

    /// Exigir motivo escrito na exclusao. Escolhido ao criar a tabela.
    pub fn com_motivo_obrigatorio(mut self, exigir: bool) -> Schema {
        self.motivo_obrigatorio = exigir;
        self
    }

    pub fn motivo_obrigatorio(&self) -> bool {
        self.motivo_obrigatorio
    }'''
assert velho3 in s
s = s.replace(velho3, novo3, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
