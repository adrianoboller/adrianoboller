# Wire config to the cache
# 29/08 00:35

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
alvo = '''    pub fn novo(config: Config) -> Result<Arc<Servidor>> {
        let instancia = Instancia::nova(&config.base)?;'''
novo = '''    pub fn novo(config: Config) -> Result<Arc<Servidor>> {
        // `recursos.cache_paginas` estava no config.json e na documentacao
        // desde a 0.13.0 -- e nao era lido por ninguem, porque o cache nao
        // existia. Agora existe, e o campo passa a valer. Tem de ser aqui,
        // ANTES de a primeira tabela abrir: o teto vale para o que abrir
        // daqui para a frente.
        phxsql_store::ndx::definir_cache_paginas(config.recursos.cache_paginas);
        let instancia = Instancia::nova(&config.base)?;'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/config.rs")
s = p.read_text()
s = s.replace('''    /// Paginas do `.ndx` mantidas em memoria. Cada uma tem 4 KiB.
    pub cache_paginas: usize,''','''    /// Paginas do `.ndx` mantidas em memoria, por arquivo aberto. Cada uma tem
    /// 4 KiB, entao 2.048 dao 8 MiB por tabela aberta.
    ///
    /// O padrao saiu de uma varredura de quatro tamanhos, em
    /// `docs/DESEMPENHO.md` §2.1: 2.048 e o joelho da curva.
    pub cache_paginas: usize,''',1)
s = s.replace("            cache_paginas: 4_096,", "            cache_paginas: 2_048,", 1)
p.write_text(s)
print("ok")
