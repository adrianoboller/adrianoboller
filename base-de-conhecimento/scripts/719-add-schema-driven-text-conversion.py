# Add schema-driven text conversion
# 28/08 19:23

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let (itens, formato) = match p.campo("texto").and_then(Json::texto) {
            Some(texto) => {
                let f = match p.texto_ou("formato", "").trim() {
                    "" | "auto" => crate::importar::adivinhar(texto),
                    outro => crate::importar::Formato::de_texto(outro)?,
                };
                let carga = crate::importar::ler(texto, f)?;
                (carga.para_json(), f.nome().to_string())
            }
            None => ('''
novo='''        let (itens, formato, de_texto) = match p.campo("texto").and_then(Json::texto) {
            Some(texto) => {
                let f = match p.texto_ou("formato", "").trim() {
                    "" | "auto" => crate::importar::adivinhar(texto),
                    outro => crate::importar::Formato::de_texto(outro)?,
                };
                let carga = crate::importar::ler(texto, f)?;
                // JSON ja vem tipado; os outros quatro sao texto puro, e a
                // conversao passa a ser dirigida pelo esquema.
                let texto_puro = f != crate::importar::Formato::Json;
                (carga.para_json(), f.nome().to_string(), texto_puro)
            }
            None => ('''
assert velho in s
s=s.replace(velho,novo,1)

s=s.replace('''                "lista".to_string(),
            ),
        };''','''                "lista".to_string(),
                false,
            ),
        };''',1)

s=s.replace('''            match json_para_linha(item, t.esquema()) {''',
'''            let convertida = if de_texto {
                crate::valores::json_para_linha_de_texto(item, t.esquema())
            } else {
                json_para_linha(item, t.esquema())
            };
            match convertida {''',1)
io.open(p,'w',encoding='utf-8').write(s)
