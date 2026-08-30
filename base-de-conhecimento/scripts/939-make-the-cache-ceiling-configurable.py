# Make the cache ceiling configurable
# 29/08 00:35

import pathlib
p = pathlib.Path("crates/phxsql-store/src/ndx.rs")
s = p.read_text()
alvo = '''/// Quantas paginas ficam em RAM por arquivo `.ndx` aberto.
///
/// 512 paginas de 4 KiB dao 2 MiB por tabela aberta. O servidor abre e fecha a
/// tabela a cada operacao, entao esse teto vale enquanto a operacao dura -- e a
/// operacao que importa aqui e a carga em lote, que insere milhares de linhas
/// dentro de uma unica abertura.
const PAGINAS_EM_CACHE: usize = 2048;'''
novo = '''/// Quantas paginas ficam em RAM por arquivo `.ndx` aberto.
///
/// 2.048 paginas de 4 KiB dao 8 MiB por tabela aberta. O numero saiu de uma
/// varredura de quatro tamanhos (`--example ordem-da-chave`, e a tabela esta em
/// `docs/DESEMPENHO.md` §2.1): 2.048 e o joelho -- dobrar de novo compra 0,8 us
/// por linha e custa mais 8 MiB.
///
/// O servidor abre e fecha a tabela a cada operacao, entao o teto vale enquanto
/// a operacao dura -- e a operacao que importa aqui e a carga em lote, que
/// insere milhares de linhas dentro de uma unica abertura.
const PAGINAS_PADRAO: usize = 2048;

/// O teto vigente, que o `config.json` ajusta em `recursos.cache_paginas`.
///
/// # Por que um global, e nao um parametro
///
/// E um teto de RAM do PROCESSO, escolhido uma vez no arranque e nunca por
/// tabela. Como parametro, ele teria de atravessar quatro camadas de API --
/// servidor, instancia, database, tabela -- so para chegar aqui, e todas as
/// quatro passariam a carregar um numero que nao e assunto delas.
static PAGINAS_EM_CACHE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(PAGINAS_PADRAO);

/// Ajusta o teto do cache de paginas do `.ndx`, em paginas.
///
/// Vale para os arquivos abertos DAQUI PARA A FRENTE: quem ja esta aberto
/// continua com o teto que tinha. Como isto e chamado no arranque, antes de a
/// primeira tabela abrir, na pratica vale para tudo.
///
/// Zero e recusado -- zero seria "sem cache", e quem quer isso desliga por
/// medida e nao por acidente de digitacao. Fica no padrao.
pub fn definir_cache_paginas(paginas: usize) {
    if paginas > 0 {
        PAGINAS_EM_CACHE.store(paginas, std::sync::atomic::Ordering::Relaxed);
    }
}

/// O teto vigente, em paginas.
pub fn cache_paginas() -> usize {
    PAGINAS_EM_CACHE.load(std::sync::atomic::Ordering::Relaxed)
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
s = s.replace("cache: CachePaginas::nova(PAGINAS_EM_CACHE),", "cache: CachePaginas::nova(cache_paginas()),")
p.write_text(s)
print("ok", s.count("CachePaginas::nova(cache_paginas())"))
