# Add com_max_arquivos and reorder
# 28/08 10:51

import pathlib
p = pathlib.Path('crates/phxsql-core/src/paginacao.rs')
s = p.read_text()
v = '''    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).
    pub fn com_digitos(mut self, digitos: u8) -> Result<Paginacao> {
        self.digitos = digitos;
        self.validada()
    }'''
n = '''    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).
    pub fn com_digitos(mut self, digitos: u8) -> Result<Paginacao> {
        self.digitos = digitos;
        self.validada()
    }

    /// Muda o teto de volumes, ja com a largura do sufixo que vigora agora.
    ///
    /// Existe por causa da ordem: `nova` confere o teto contra os tres digitos
    /// do padrao, entao pedir 9999 volumes ali e recusado antes de o quarto
    /// digito existir. Com este metodo a largura entra primeiro e o teto
    /// depois, que e a ordem em que os dois fazem sentido.
    pub fn com_max_arquivos(mut self, max_arquivos: u32) -> Result<Paginacao> {
        self.max_arquivos = max_arquivos;
        self.validada()
    }'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''        let cabem = 10u32.pow(digitos as u32) - 1;
        let max = match j.inteiro_ou("max_arquivos", 0).max(0) as u32 {
            0 => cabem,
            outro => outro,
        };
        esquema.com_paginacao(Paginacao::nova(por_arquivo as u64, max)?.com_digitos(digitos)?)'''
n = '''        let cabem = 10u32.pow(digitos as u32) - 1;
        let max = match j.inteiro_ou("max_arquivos", 0).max(0) as u32 {
            0 => cabem,
            outro => outro,
        };
        // A largura do sufixo entra ANTES do teto: `nova` confere o teto contra
        // os tres digitos do padrao, e um teto de 9999 seria recusado antes de
        // o quarto digito existir.
        esquema.com_paginacao(
            Paginacao::nova(por_arquivo as u64, 1)?
                .com_digitos(digitos)?
                .com_max_arquivos(max)?,
        )'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')
