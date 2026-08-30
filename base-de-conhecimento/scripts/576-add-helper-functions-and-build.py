# Add helper functions and build
# 28/08 17:40

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho='''pub fn linha_para_json(linha: &[Value], esquema: &Schema) -> Json {'''
novo='''/// As colunas de um esquema, no minimo que uma grade precisa para desenhar.
///
/// Marca a coluna de sistema com `"sistema": true` para a tela poder trata-la
/// como o que ela e: nao se digita, nao se edita, e quem manda nela e o botao
/// de excluir. Sem essa marca, ela apareceria como mais um campo de formulario.
pub fn colunas_para_json(esquema: &Schema) -> Json {
    let sistema = esquema.coluna_softdeleted();
    Json::Lista(
        esquema
            .colunas()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                Json::objeto(vec![
                    ("nome", Json::texto_de(&c.nome)),
                    ("rotulo", Json::texto_de(c.rotulo())),
                    ("tipo", Json::texto_de(format!("{:?}", c.ty))),
                    ("nullable", Json::Bool(c.nullable)),
                    ("sistema", Json::Bool(Some(i) == sistema)),
                ])
            })
            .collect(),
    )
}

pub fn linha_para_json(linha: &[Value], esquema: &Schema) -> Json {'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)
