# Fix the header style and the missing tblGrid
# 28/08 16:58

p='crates/phxsql-server/src/exportar.rs'
s=open(p).read()

# --- 1. Os indices de estilo viram constantes com nome.
#     Eu tinha escrito `1` para o cabecalho, e 1 e o "texto listrado": o
#     cabecalho saia com a cor da zebra e sem negrito. O leitor independente
#     pegou; o meu proprio codigo nao tinha como pegar.
a='''/// Dias entre 1899-12-30 (a epoca do Excel(R)) e 1970-01-01 (a nossa).
const EPOCA_PLANILHA: i64 = 25_569;'''
b='''/// Dias entre 1899-12-30 (a epoca do Excel(R)) e 1970-01-01 (a nossa).
const EPOCA_PLANILHA: i64 = 25_569;

/// Os indices do `cellXfs` da folha de estilos, por nome.
///
/// Numero solto aqui ja custou caro: o cabecalho apontava para `1`, que e o
/// "texto listrado", e saia com a cor da zebra e sem negrito. O Excel(R) nao
/// reclama de indice errado -- ele obedece. Quem achou foi um leitor
/// independente lendo o arquivo de volta, e por isso os indices agora tem
/// nome e ha teste que confere a correspondencia.
mod estilo {
    pub const TEXTO: u32 = 0;
    pub const TEXTO_ZEBRA: u32 = 1;
    pub const INTEIRO: u32 = 2;
    pub const INTEIRO_ZEBRA: u32 = 3;
    pub const TITULO: u32 = 4;
    pub const SUBTITULO: u32 = 5;
    pub const DECIMAL: u32 = 6;
    pub const DECIMAL_ZEBRA: u32 = 7;
    pub const DATA: u32 = 8;
    pub const DATA_ZEBRA: u32 = 9;
    pub const INSTANTE: u32 = 10;
    pub const INSTANTE_ZEBRA: u32 = 11;
    pub const CABECALHO: u32 = 12;
    /// Quantos `xf` a folha de estilos declara. Um a mais ou a menos aqui e o
    /// Excel(R) recusando o arquivo inteiro.
    pub const QUANTOS: usize = 13;
}'''
assert a in s; s=s.replace(a,b,1)

# --- usa os nomes
s=s.replace('celula_texto("A1", &self.titulo, 4)','celula_texto("A1", &self.titulo, estilo::TITULO)',1)
s=s.replace('celula_texto("A2", &self.subtitulo, 5)','celula_texto("A2", &self.subtitulo, estilo::SUBTITULO)',1)
s=s.replace('&celula_texto(&format!("{}4", coluna_a1(i)), &c.nome, 1)',
            '&celula_texto(&format!("{}4", coluna_a1(i)), &c.nome, estilo::CABECALHO)',1)
a='''        Value::Date(d) => format!(
            "<c r=\\"{ref_}\\" s=\\"{}\\"><v>{}</v></c>",
            base(8, 9),'''
b='''        Value::Date(d) => format!(
            "<c r=\\"{ref_}\\" s=\\"{}\\"><v>{}</v></c>",
            base(estilo::DATA, estilo::DATA_ZEBRA),'''
assert a in s; s=s.replace(a,b,1)
s=s.replace('format!("<c r=\\"{ref_}\\" s=\\"{}\\"><v>{dias:.6}</v></c>", base(10, 11))',
            'format!(\n                "<c r=\\"{ref_}\\" s=\\"{}\\"><v>{dias:.6}</v></c>",\n                base(estilo::INSTANTE, estilo::INSTANTE_ZEBRA)\n            )',1)
s=s.replace('                base(6, 7),\n                decimal_para_texto(*d, escala)',
            '                base(estilo::DECIMAL, estilo::DECIMAL_ZEBRA),\n                decimal_para_texto(*d, escala)',1)
s=s.replace('format!("<c r=\\"{ref_}\\" s=\\"{}\\"><v>{i}</v></c>", base(2, 3))',
            'format!(\n            "<c r=\\"{ref_}\\" s=\\"{}\\"><v>{i}</v></c>",\n            base(estilo::INTEIRO, estilo::INTEIRO_ZEBRA)\n        )',1)
s=s.replace('format!("<c r=\\"{ref_}\\" s=\\"{}\\"><v>{u}</v></c>", base(2, 3))',
            'format!(\n            "<c r=\\"{ref_}\\" s=\\"{}\\"><v>{u}</v></c>",\n            base(estilo::INTEIRO, estilo::INTEIRO_ZEBRA)\n        )',1)
s=s.replace('format!("<c r=\\"{ref_}\\" s=\\"{}\\"><v>{r}</v></c>", base(6, 7))',
            'format!(\n                "<c r=\\"{ref_}\\" s=\\"{}\\"><v>{r}</v></c>",\n                base(estilo::DECIMAL, estilo::DECIMAL_ZEBRA)\n            )',1)
s=s.replace('outro => celula_texto(ref_, &texto_de(outro, t, false), base(0, 1)),',
            'outro => celula_texto(\n            ref_,\n            &texto_de(outro, t, false),\n            base(estilo::TEXTO, estilo::TEXTO_ZEBRA),\n        ),',1)
a='''    let base = |normal: u32, listrado: u32| if zebra { listrado } else { normal };'''
b='''    let base = |normal: u32, listrado: u32| if zebra { listrado } else { normal };
    let _ = estilo::QUANTOS;'''
assert a in s; s=s.replace(a,b,1)

# --- 2. o xf do cabecalho entra na folha de estilos
a='''<cellXfs count="12">'''
b='''<cellXfs count="13">'''
assert a in s; s=s.replace(a,b,1)
a='''<xf numFmtId="172" fontId="0" fillId="3" borderId="1" xfId="0" applyNumberFormat="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>
</cellXfs>'''
b='''<xf numFmtId="172" fontId="0" fillId="3" borderId="1" xfId="0" applyNumberFormat="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment horizontal="center"/></xf>
<xf numFmtId="0" fontId="1" fillId="2" borderId="1" xfId="0" applyFont="1" applyFill="1" applyBorder="1" applyAlignment="1"><alignment vertical="center"/></xf>
</cellXfs>'''
assert a in s; s=s.replace(a,b,1)

# --- 3. o comentario mentiroso sai
a='''/// O cabecalho do XLSX precisa do estilo 1 (fundo escuro, texto branco). Ele
/// nao esta no `cellXfs` acima por engano de contagem -- esta: o indice 1 e o
/// "texto listrado", e o cabecalho usa um `xf` proprio que o `celula_texto`
/// recebe por parametro. Este teste trava a correspondencia.
#[cfg(test)]'''
b='''#[cfg(test)]'''
assert a in s; s=s.replace(a,b,1)

# --- 4. o docx ganha o <w:tblGrid>, que e obrigatorio
a='''        d.push_str(
            "<w:tbl><w:tblPr><w:tblStyle w:val=\\"a\\"/>\\
             <w:tblW w:w=\\"5000\\" w:type=\\"pct\\"/>\\'''
b='''        // O `tblGrid` e OBRIGATORIO dentro de `w:tbl`, e sem ele o documento
        // nao e OOXML valido -- um leitor rigoroso recusa a tabela inteira. O
        // Word(R) tolera e por isso o defeito passaria despercebido ate alguem
        // abrir o arquivo noutro programa.
        let largura_col = 9_360 / self.colunas.len().max(1);
        d.push_str(
            "<w:tbl><w:tblPr><w:tblStyle w:val=\\"a\\"/>\\
             <w:tblW w:w=\\"5000\\" w:type=\\"pct\\"/>\\'''
assert a in s; s=s.replace(a,b,1)
a='''             </w:tblBorders></w:tblPr>",
        );
        d.push_str("<w:tr><w:trPr><w:tblHeader/></w:trPr>");'''
b='''             </w:tblBorders></w:tblPr>",
        );
        d.push_str("<w:tblGrid>");
        for _ in &self.colunas {
            d.push_str(&format!("<w:gridCol w:w=\\"{largura_col}\\"/>"));
        }
        d.push_str("</w:tblGrid>");
        // `tblHeader` repete o cabecalho em toda pagina de uma tabela longa.
        d.push_str("<w:tr><w:trPr><w:tblHeader/></w:trPr>");'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
