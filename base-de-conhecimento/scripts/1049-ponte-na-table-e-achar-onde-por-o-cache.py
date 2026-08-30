# Ponte na Table e achar onde por o cache
# 29/08 03:46

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    /// Total de eventos registrados no diario.
    pub fn eventos(&mut self) -> Result<u64> {'''
novo = '''    /// Onde a ultima leitura do diario parou. Ver [`crate::log::MarcaDoDiario`].
    ///
    /// Existe para quem le o diario em lotes seguidos e nao mantem a tabela
    /// aberta entre eles -- o servidor, na replicacao.
    pub fn marca_do_diario(&self) -> Option<crate::log::MarcaDoDiario> {
        self.log.marca()
    }

    /// Aceita a dica de onde comecar a proxima leitura do diario.
    pub fn definir_marca_do_diario(&mut self, marca: Option<crate::log::MarcaDoDiario>) {
        self.log.definir_marca(marca);
    }

    /// Total de eventos registrados no diario.
    pub fn eventos(&mut self) -> Result<u64> {'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('table ok')
