# Complete arity on insert and update
# 28/08 17:30

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

# inserir: completar antes de numerar
velho='''    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {
        self.conferir_aridade(valores)?;

        // A sequencia entra ANTES das chaves: se a coluna estiver num indice,
        // a chave tem de ser a do numero que vai ser gravado, nao a do nulo.
        let proprios;
        let valores = match self.numerar(valores, None)? {'''
novo='''    pub fn inserir(&mut self, valores: &[Value]) -> Result<RowId> {
        self.conferir_aridade(valores)?;
        let completos;
        let valores = match self.completar(valores, None) {
            Some(v) => {
                completos = v;
                &completos[..]
            }
            None => valores,
        };

        // A sequencia entra ANTES das chaves: se a coluna estiver num indice,
        // a chave tem de ser a do numero que vai ser gravado, nao a do nulo.
        let proprios;
        let valores = match self.numerar(valores, None)? {'''
assert velho in s
s=s.replace(velho,novo,1)

# atualizar: completar com o valor anterior
velho2='''        let valores_antigos = self.decodificar(&antigo, false)?;

        // Nulo na coluna de sequencia guarda o numero que a linha ja tinha.
        let proprios;'''
novo2='''        let valores_antigos = self.decodificar(&antigo, false)?;

        // Sem a coluna de sistema nos valores, herda a marca da linha: um
        // `atualizar` de rotina nao ressuscita linha excluida por descuido.
        let completos;
        let valores = match self.completar(valores, Some(&valores_antigos)) {
            Some(v) => {
                completos = v;
                &completos[..]
            }
            None => valores,
        };

        // Nulo na coluna de sequencia guarda o numero que a linha ja tinha.
        let proprios;'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')
