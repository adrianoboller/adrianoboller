# Read the partition mode from JSON
# 28/08 11:17

import pathlib
p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''        let cabem = 10u32.pow(digitos as u32) - 1;'''
n = '''        // A particao por periodo aponta a coluna por NOME, como os indices --
        // posicao e detalhe de implementacao.
        let modo = match j.texto_ou("particao", "").trim() {
            "" | "quantidade" | "faixa" => ModoParticao::PorQuantidade,
            nome_periodo => {
                let coluna = j.texto_ou("particao_coluna", "").trim().to_string();
                let i = colunas
                    .iter()
                    .position(|c| c.nome == coluna)
                    .ok_or_else(|| {
                        PhxError::Esquema(format!(
                            "a particao {nome_periodo} precisa de \\"particao_coluna\\" \\
                             com o nome de uma coluna de data; recebi {coluna:?}"
                        ))
                    })?;
                ModoParticao::PorPeriodo {
                    coluna: i as u16,
                    periodo: Periodo::de_nome(nome_periodo)?,
                }
            }
        };

        let cabem = 10u32.pow(digitos as u32) - 1;'''
assert s.count(v) == 1
s = s.replace(v, n)
s = s.replace('use phxsql_core::paginacao::{Paginacao, DIGITOS_PADRAO};',
              'use phxsql_core::paginacao::{ModoParticao, Paginacao, Periodo, DIGITOS_PADRAO};')
p.write_text(s)

p = pathlib.Path('crates/phxsql-cli/src/main.rs')
s = p.read_text()
v = 'esquema.com_paginacao(Paginacao::nova(2, 99)?)'
assert s.count(v) == 1
p.write_text(s.replace(v, 'esquema.com_paginacao(Paginacao::nova(2, 99)?)?'))
print('ok')
