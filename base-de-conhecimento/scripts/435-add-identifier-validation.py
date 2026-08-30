# Add identifier validation
# 28/08 14:47

p='crates/phxsql-server/src/dblink/mod.rs'
s=open(p).read()
a='''/// Um nome de tabela ou coluna vindo da tela, protegido com crase.'''
b='''/// Um nome de tabela, coluna ou base vindo da tela, conferido antes de virar
/// SQL.
///
/// A defesa e recusar, e nao escapar. Escapar aspas exige saber em que modo o
/// outro servidor esta -- com `NO_BACKSLASH_ESCAPES` a contrabarra deixa de
/// escapar, e a mesma regra que protegia passa a nao proteger. Nome de objeto
/// nao precisa de aspa, crase, contrabarra nem quebra de linha, entao nada
/// disso entra.
pub fn nome_seguro(nome: &str) -> Result<String> {
    let n = nome.trim();
    if n.is_empty() {
        return Err(PhxError::Esquema("nome vazio".into()));
    }
    if n.len() > 128 {
        return Err(PhxError::Esquema(format!(
            "nome longo demais: {n:?} (o MySQL(R) para em 64)"
        )));
    }
    if n.chars()
        .any(|c| c.is_control() || matches!(c, '`' | '\\'' | '"' | '\\\\'))
    {
        return Err(PhxError::Esquema(format!(
            "nome com caractere que nao vale em identificador: {n:?}"
        )));
    }
    Ok(n.to_string())
}

/// Um nome que precisa virar TEXTO no SQL, como em `TABLE_SCHEMA = '...'`.
///
/// Passa pelo mesmo crivo do identificador e so depois vira literal: sem aspa
/// nem contrabarra dentro, as aspas de fora fecham onde devem em qualquer modo
/// do servidor.
pub fn literal(valor: &str) -> Result<String> {
    Ok(format!("'{}'", nome_seguro(valor)?))
}

/// Um nome de tabela ou coluna vindo da tela, protegido com crase.'''
assert a in s; s=s.replace(a,b,1)

a='''    #[test]
    fn a_crase_no_nome_e_escapada() {'''
b='''    #[test]
    fn nome_seguro_recusa_o_que_emendaria_sql() {
        for ruim in [
            "cli`entes",
            "cli'entes",
            "cli\\"entes",
            "cli\\\\entes",
            "cli\\nentes",
            "  ",
        ] {
            assert!(nome_seguro(ruim).is_err(), "aceitou {ruim:?}");
        }
        assert_eq!(nome_seguro(" clientes ").unwrap(), "clientes");
        // Nome com espaco e acento continua valendo: sao legais no MySQL(R) e
        // a crase de fora resolve.
        assert_eq!(nome_seguro("Notas Fiscais").unwrap(), "Notas Fiscais");
        assert_eq!(literal("loja").unwrap(), "'loja'");
    }

    #[test]
    fn a_crase_no_nome_e_escapada() {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
