# Bump PSCH version and add constant
# 28/08 17:22

import io
p='crates/phxsql-core/src/schema.rs'
s=io.open(p,encoding='utf-8').read()

# 1. Versao do esquema: 3 -> 4
velho = '''/// A 3 acrescentou os metadados de coluna (`id`, `caption`, `descricao`,
/// `mascara`), o marcador de chave primaria no indice e o modo de particao.
/// A leitura ainda aceita a 2: tabela gravada antes abre, ganha um `id` v7
/// sorteado na hora e os textos vazios. Escrever, so na 3.
const VERSAO_ESQUEMA: u16 = 3;
const VERSAO_ESQUEMA_MINIMA: u16 = 2;'''
novo = '''/// A 3 acrescentou os metadados de coluna (`id`, `caption`, `descricao`,
/// `mascara`), o marcador de chave primaria no indice e o modo de particao.
/// A 4 acrescentou a coluna de sistema [`COLUNA_SOFTDELETED`] e o sinal de
/// motivo obrigatorio.
///
/// A leitura ainda aceita a 2: tabela gravada antes abre, ganha um `id` v7
/// sorteado na hora e os textos vazios. Escrever, so na 4.
///
/// # Por que a v3 nao ganha a coluna ao ser lida
///
/// A coluna de sistema entra em [`Schema::new`], que e o caminho de CRIAR
/// tabela. A leitura do disco usa outro caminho, que nao acrescenta nada: o
/// `payload_len` sai da lista de colunas gravada, e uma coluna a mais
/// deslocaria o offset de todas as seguintes. Uma tabela v3 continua legivel
/// exatamente como esta -- so nao tem exclusao suave, e a mensagem de erro
/// diz isso em vez de ler lixo.
const VERSAO_ESQUEMA: u16 = 4;
const VERSAO_ESQUEMA_MINIMA: u16 = 2;

/// Nome da coluna de sistema que marca a linha como excluida sem excluir.
///
/// Toda tabela criada a partir da v4 tem esta coluna, no FIM da lista: no fim
/// porque assim os offsets das colunas do usuario nao mudam de lugar quando
/// ela entra, e quem monta a linha posicionalmente pode continuar mandando so
/// as colunas que declarou.
pub const COLUNA_SOFTDELETED: &str = "softdeleted";'''
assert velho in s
s = s.replace(velho, novo, 1)
io.open(p,'w',encoding='utf-8').write(s)
print('versao ok')
