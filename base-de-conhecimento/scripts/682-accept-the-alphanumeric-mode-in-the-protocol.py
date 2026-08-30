# Accept the alphanumeric mode in the protocol
# 28/08 18:54

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let modo = match j.texto_ou("particao", "").trim() {
            "" | "quantidade" | "faixa" => ModoParticao::PorQuantidade,
            nome_periodo => {'''
novo='''        let modo = match j.texto_ou("particao", "").trim() {
            "" | "quantidade" | "faixa" => ModoParticao::PorQuantidade,
            "letra" | "alfanumerica" | "alfanumerico" => {
                let coluna = j.texto_ou("particao_coluna", "").trim().to_string();
                let i = esquema
                    .colunas()
                    .iter()
                    .position(|c| c.nome == coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a particao alfanumerica precisa de \\"particao_coluna\\" \\
                             com o nome da coluna de referencia; recebi {coluna:?}"
                        ))
                    })?;
                ModoParticao::PorLetra { coluna: i as u16 }
            }
            nome_periodo => {'''
assert velho in s
s=s.replace(velho,novo,1)

velho2='''        let cabem = 10u32.pow(digitos as u32) - 1;
        let max = match j.inteiro_ou("max_arquivos", 0).max(0) as u32 {
            0 => cabem,
            outro => outro,
        };'''
novo2='''        // Na alfanumerica o numero de volumes NAO se escolhe: sao os 37
        // baldes, e o construtor cuida do sufixo. Deixar a tela mandar um teto
        // aqui so criaria um jeito de pedir uma tabela que o validador recusa.
        if let ModoParticao::PorLetra { coluna } = modo {
            return esquema.com_paginacao(Paginacao::por_letra(por_arquivo as u64, coluna)?);
        }

        let cabem = 10u32.pow(digitos as u32) - 1;
        let max = match j.inteiro_ou("max_arquivos", 0).max(0) as u32 {
            0 => cabem,
            outro => outro,
        };'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)
