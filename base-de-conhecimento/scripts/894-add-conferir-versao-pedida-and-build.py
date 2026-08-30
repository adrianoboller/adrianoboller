# Add conferir_versao_pedida and build
# 28/08 23:53

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
alvo = '''fn trava_envenenada() -> PhxError {
    PhxError::Corrompido("uma operacao anterior entrou em panico e deixou a trava suja".into())
}
'''
novo = '''fn trava_envenenada() -> PhxError {
    PhxError::Corrompido("uma operacao anterior entrou em panico e deixou a trava suja".into())
}

/// A guarda de conflito de escrita, quando o cliente pede.
///
/// # Por que a conferencia e pedida, e nao imposta
///
/// Imposta, todo cliente escrito antes desta versao pararia de gravar de
/// um dia para o outro -- e o que ele estaria recebendo nao e protecao, e um
/// erro que ele nao sabe tratar. Pedida, quem manda a versao ganha a garantia
/// na hora e quem nao manda continua com o comportamento de sempre: a ultima
/// gravacao vence.
///
/// A interface web manda sempre, porque ali existe gente do outro lado e
/// existe a janela de minutos entre abrir a ficha e clicar em salvar. E onde
/// o conflito de fato acontece.
///
/// Zero e ausente sao a mesma coisa: a versao de um registro vivo comeca em
/// 1, entao o zero nao tira nenhum valor legitimo do caminho.
fn conferir_versao_pedida(t: &mut Table, p: &Json, rowid: RowId) -> Result<()> {
    let esperada = p.inteiro_ou("versao", 0).max(0) as u64;
    if esperada == 0 {
        return Ok(());
    }
    t.conferir_versao(rowid, esperada)
}
'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")
