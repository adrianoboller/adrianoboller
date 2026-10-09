#[derive(serde::Deserialize, Clone, PartialEq)]
pub struct Linha { pub id: u32, pub nome: String, pub status: String }
pub const DADOS: &str = include_str!("dados.json");
pub fn t(k: &str) -> &'static str {
    match k { "titulo"=>"Clientes", "filtro"=>"Filtrar", "nome"=>"Nome", "status"=>"Situacao",
      "email"=>"E-mail", "fone"=>"Telefone", "cidade"=>"Cidade", "uf"=>"UF", "doc"=>"Documento",
      "salvar"=>"Salvar", "obrigatorio"=>"Nome e obrigatorio", "salvo"=>"Salvo", _=>"?" }
}
