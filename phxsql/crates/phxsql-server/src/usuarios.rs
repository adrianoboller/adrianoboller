//! Cadastro de usuarios e poder de cada um sobre cada base.
//!
//! Tudo mora no `config.json`, conforme pedido -- com uma diferenca: a senha
//! e guardada como HASH, nunca em texto puro. Ver [`phxsql_core::senha`].
//!
//! ```json
//! "root":  { "login": "root", "senha_hash": "pbkdf2-sha256$..." },
//! "usuarios": [
//!   {
//!     "id": 2,
//!     "nome": "Adriano Boller",
//!     "login": "adriano",
//!     "senha_hash": "pbkdf2-sha256$210000$...$...",
//!     "email": "adriano@empresa.com.br",
//!     "telefone": "+55 47 99999-0000",
//!     "supervisor": false,
//!     "ativo": true,
//!     "bases": {
//!       "*": { "ler": true, "verificar": true },
//!       "Z": { "ler": true, "inserir": true, "alterar": true, "diario": true }
//!     }
//!   }
//! ]
//! ```
//!
//! # Duas regras que valem a pena conhecer
//!
//! * **Nega por omissao.** Atividade que nao aparece na base e `false`. Base
//!   que nao aparece cai no `"*"`; sem `"*"`, o acesso e negado.
//! * **Supervisor pode tudo, em toda base.** O `root` e sempre supervisor.

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::senha;

/// O que um usuario pode fazer numa base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atividade {
    /// Ler dados: `ler`, `varrer`, `buscar`, `esquema`, `tabelas`, `bancos`.
    Ler,
    Inserir,
    Alterar,
    Excluir,
    /// Criar database, schema ou tabela.
    Criar,
    /// Recriar o `.ndx`.
    Reindexar,
    /// Ver o diario da tabela.
    Diario,
    /// Conferir a integridade.
    Verificar,
    /// Ver o log de acessos, os IPs e a configuracao.
    Administrar,
    /// Pedir o fluxo de replicacao.
    Replicar,
}

impl Atividade {
    pub fn nome(self) -> &'static str {
        match self {
            Atividade::Ler => "ler",
            Atividade::Inserir => "inserir",
            Atividade::Alterar => "alterar",
            Atividade::Excluir => "excluir",
            Atividade::Criar => "criar",
            Atividade::Reindexar => "reindexar",
            Atividade::Diario => "diario",
            Atividade::Verificar => "verificar",
            Atividade::Administrar => "administrar",
            Atividade::Replicar => "replicar",
        }
    }

    pub const TODAS: [Atividade; 10] = [
        Atividade::Ler,
        Atividade::Inserir,
        Atividade::Alterar,
        Atividade::Excluir,
        Atividade::Criar,
        Atividade::Reindexar,
        Atividade::Diario,
        Atividade::Verificar,
        Atividade::Administrar,
        Atividade::Replicar,
    ];

    /// Qual atividade uma operacao do protocolo exige.
    ///
    /// `None` significa que a operacao nao exige poder nenhum alem de estar
    /// autenticado -- e o caso do `ping` e do `login`.
    pub fn da_operacao(op: &str) -> Option<Atividade> {
        Some(match op {
            "ping" | "login" | "quem_sou" => return None,
            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "buscar" => Atividade::Ler,
            "inserir" => Atividade::Inserir,
            "atualizar" => Atividade::Alterar,
            "excluir" => Atividade::Excluir,
            "criar_database" | "criar_schema" => Atividade::Criar,
            "reindexar" => Atividade::Reindexar,
            "diario" => Atividade::Diario,
            "verificar" => Atividade::Verificar,
            "acessos" | "ips" | "config" | "usuarios" => Atividade::Administrar,
            "posicao" | "replicar" => Atividade::Replicar,
            // Operacao desconhecida exige o maior poder: nega por omissao.
            _ => Atividade::Administrar,
        })
    }
}

/// As dez permissoes de uma base. Tudo comeca em `false`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Permissoes {
    pub ler: bool,
    pub inserir: bool,
    pub alterar: bool,
    pub excluir: bool,
    pub criar: bool,
    pub reindexar: bool,
    pub diario: bool,
    pub verificar: bool,
    pub administrar: bool,
    pub replicar: bool,
}

impl Permissoes {
    pub fn tudo() -> Permissoes {
        Permissoes {
            ler: true,
            inserir: true,
            alterar: true,
            excluir: true,
            criar: true,
            reindexar: true,
            diario: true,
            verificar: true,
            administrar: true,
            replicar: true,
        }
    }

    pub fn pode(&self, a: Atividade) -> bool {
        match a {
            Atividade::Ler => self.ler,
            Atividade::Inserir => self.inserir,
            Atividade::Alterar => self.alterar,
            Atividade::Excluir => self.excluir,
            Atividade::Criar => self.criar,
            Atividade::Reindexar => self.reindexar,
            Atividade::Diario => self.diario,
            Atividade::Verificar => self.verificar,
            Atividade::Administrar => self.administrar,
            Atividade::Replicar => self.replicar,
        }
    }

    fn de_json(j: &Json) -> Permissoes {
        Permissoes {
            ler: j.booleano_ou("ler", false),
            inserir: j.booleano_ou("inserir", false),
            alterar: j.booleano_ou("alterar", false),
            excluir: j.booleano_ou("excluir", false),
            criar: j.booleano_ou("criar", false),
            reindexar: j.booleano_ou("reindexar", false),
            diario: j.booleano_ou("diario", false),
            verificar: j.booleano_ou("verificar", false),
            administrar: j.booleano_ou("administrar", false),
            replicar: j.booleano_ou("replicar", false),
        }
    }

    pub fn para_json(&self) -> Json {
        Json::objeto(
            Atividade::TODAS
                .iter()
                .map(|a| (a.nome(), Json::Bool(self.pode(*a))))
                .collect(),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Usuario {
    /// Identificacao numerica, gravada no `.log` de cada tabela como autor da
    /// operacao. Se omitida no `config.json`, sai do CRC-32 do login.
    pub id: u32,
    pub nome: String,
    pub login: String,
    pub senha_hash: String,
    pub email: String,
    pub telefone: String,
    pub supervisor: bool,
    pub ativo: bool,
    /// Poder por base. A chave `"*"` vale para as bases nao listadas.
    pub bases: Vec<(String, Permissoes)>,
}

impl Usuario {
    /// A senha confere?
    pub fn senha_confere(&self, oferecida: &str) -> bool {
        self.ativo && senha::conferir(oferecida, &self.senha_hash)
    }

    /// Permissoes efetivas numa base.
    pub fn permissoes(&self, database: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == database) {
            return *p;
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == "*") {
            return *p;
        }
        Permissoes::default()
    }

    /// Pode fazer a atividade nesta base?
    pub fn pode(&self, database: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes(database).pode(atividade)
    }

    /// Ficha do usuario, sem a senha. Nunca devolve o hash.
    pub fn ficha(&self) -> Json {
        Json::objeto(vec![
            ("id", Json::de_u64(self.id as u64)),
            ("nome", Json::texto_de(&self.nome)),
            ("login", Json::texto_de(&self.login)),
            ("email", Json::texto_de(&self.email)),
            ("telefone", Json::texto_de(&self.telefone)),
            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),
            (
                "bases",
                Json::Objeto(
                    self.bases
                        .iter()
                        .map(|(b, p)| (b.clone(), p.para_json()))
                        .collect(),
                ),
            ),
        ])
    }

    fn de_json(j: &Json, avisos: &mut Vec<String>) -> Result<Usuario> {
        let login = j.texto_ou("login", "").trim().to_string();
        if login.is_empty() {
            return Err(PhxError::Esquema("usuario sem login".into()));
        }

        let hash = extrair_hash(j, &login, avisos)?;

        let bases = match j.campo("bases") {
            Some(Json::Objeto(pares)) => pares
                .iter()
                .map(|(base, perm)| (base.clone(), Permissoes::de_json(perm)))
                .collect(),
            _ => Vec::new(),
        };

        let id = j
            .campo("id")
            .and_then(Json::inteiro)
            .filter(|n| *n > 0 && *n <= u32::MAX as i64)
            .map(|n| n as u32)
            .unwrap_or_else(|| phxsql_core::crc::crc32(login.as_bytes()).max(1));

        Ok(Usuario {
            id,
            nome: j.texto_ou("nome", &login).to_string(),
            login,
            senha_hash: hash,
            email: j.texto_ou("email", "").to_string(),
            telefone: j.texto_ou("telefone", "").to_string(),
            supervisor: j.booleano_ou("supervisor", false),
            ativo: j.booleano_ou("ativo", true),
            bases,
        })
    }
}

/// Aceita `senha_hash` (o certo) ou `senha` em texto puro (avisando alto).
fn extrair_hash(j: &Json, login: &str, avisos: &mut Vec<String>) -> Result<String> {
    if let Some(h) = j.campo("senha_hash").and_then(Json::texto) {
        if senha::e_hash(h) {
            return Ok(h.to_string());
        }
        return Err(PhxError::Esquema(format!(
            "senha_hash de {login} esta malformada; gere com: phxsqld --senha"
        )));
    }
    if let Some(clara) = j.campo("senha").and_then(Json::texto) {
        if clara.is_empty() {
            return Err(PhxError::Esquema(format!("usuario {login} sem senha")));
        }
        avisos.push(format!(
            "usuario {login} esta com a SENHA EM TEXTO PURO no config.json. \
             Troque por senha_hash: phxsqld --senha"
        ));
        return Ok(senha::cifrar(clara));
    }
    Err(PhxError::Esquema(format!(
        "usuario {login} sem senha_hash nem senha"
    )))
}

/// O cadastro inteiro: o root e os demais.
#[derive(Debug, Clone, Default)]
pub struct Cadastro {
    pub root: Option<Usuario>,
    pub usuarios: Vec<Usuario>,
    /// Problemas que nao impedem subir, mas que precisam aparecer no arranque.
    pub avisos: Vec<String>,
}

impl Cadastro {
    pub fn de_json(config: &Json) -> Result<Cadastro> {
        let mut avisos = Vec::new();

        let root = match config.campo("root") {
            None => None,
            Some(j) => {
                let mut u = Usuario::de_json(j, &mut avisos)?;
                if u.login.is_empty() {
                    u.login = "root".to_string();
                }
                // O root e supervisor por definicao, diga o que disser o arquivo.
                u.supervisor = true;
                u.ativo = true;
                if u.id == 0 {
                    u.id = 1;
                }
                Some(u)
            }
        };

        let mut usuarios = Vec::new();
        if let Some(lista) = config.campo("usuarios").and_then(Json::lista) {
            for j in lista {
                let u = Usuario::de_json(j, &mut avisos)?;
                if usuarios.iter().any(|o: &Usuario| o.login == u.login) {
                    return Err(PhxError::Esquema(format!("login repetido: {}", u.login)));
                }
                if root.as_ref().is_some_and(|r| r.login == u.login) {
                    return Err(PhxError::Esquema(format!(
                        "o login {} colide com o root",
                        u.login
                    )));
                }
                usuarios.push(u);
            }
        }

        for u in &usuarios {
            if let Some(outro) = usuarios.iter().find(|o| o.id == u.id && o.login != u.login) {
                return Err(PhxError::Esquema(format!(
                    "id {} repetido entre {} e {}",
                    u.id, u.login, outro.login
                )));
            }
        }

        Ok(Cadastro {
            root,
            usuarios,
            avisos,
        })
    }

    /// Ha alguem cadastrado? Sem cadastro, o servidor cai no token de servico.
    pub fn vazio(&self) -> bool {
        self.root.is_none() && self.usuarios.is_empty()
    }

    pub fn por_login(&self, login: &str) -> Option<&Usuario> {
        if let Some(r) = &self.root {
            if r.login == login {
                return Some(r);
            }
        }
        self.usuarios.iter().find(|u| u.login == login)
    }

    /// Confere login e senha. Devolve o usuario so quando os dois batem e a
    /// conta esta ativa.
    ///
    /// Quando o login nao existe, ainda assim gasta o tempo de um PBKDF2, para
    /// que "usuario inexistente" e "senha errada" nao se distingam pelo relogio.
    pub fn autenticar(&self, login: &str, oferecida: &str) -> Option<&Usuario> {
        match self.por_login(login) {
            Some(u) if u.senha_confere(oferecida) => Some(u),
            Some(_) => None,
            None => {
                let _ = senha::conferir(oferecida, &senha::cifrar_com("nao-existe", 1_000));
                None
            }
        }
    }

    /// Fichas de todos, sem senha.
    pub fn fichas(&self) -> Json {
        let mut lista: Vec<Json> = Vec::new();
        if let Some(r) = &self.root {
            lista.push(r.ficha());
        }
        lista.extend(self.usuarios.iter().map(Usuario::ficha));
        Json::Lista(lista)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cadastro(txt: &str) -> Cadastro {
        Cadastro::de_json(&Json::analisar(txt).unwrap()).unwrap()
    }

    fn hash_rapido(s: &str) -> String {
        senha::cifrar_com(s, 64)
    }

    #[test]
    fn le_o_cadastro_completo() {
        let txt = format!(
            r#"{{
              "root": {{"login":"root","senha_hash":"{}"}},
              "usuarios": [{{
                "id": 7,
                "nome": "Adriano Boller",
                "login": "adriano",
                "senha_hash": "{}",
                "email": "adriano@empresa.com.br",
                "telefone": "+55 47 99999-0000",
                "supervisor": false,
                "ativo": true,
                "bases": {{
                  "*": {{"ler": true}},
                  "Z": {{"ler": true, "inserir": true, "alterar": true, "diario": true}}
                }}
              }}]
            }}"#,
            hash_rapido("raiz"),
            hash_rapido("segredo")
        );
        let c = cadastro(&txt);
        let u = c.por_login("adriano").unwrap();
        assert_eq!(u.id, 7);
        assert_eq!(u.nome, "Adriano Boller");
        assert_eq!(u.email, "adriano@empresa.com.br");
        assert_eq!(u.telefone, "+55 47 99999-0000");
        assert!(!u.supervisor);
        assert!(u.ativo);
        assert!(c.avisos.is_empty());
    }

    #[test]
    fn poder_por_base_com_curinga() {
        let txt = format!(
            r#"{{"usuarios":[{{
                 "login":"joao","senha_hash":"{}",
                 "bases":{{
                   "*":{{"ler":true}},
                   "Z":{{"ler":true,"inserir":true,"alterar":true}},
                   "W":{{}}
                 }}}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        let u = c.por_login("joao").unwrap();

        // Base listada: vale o que esta la.
        assert!(u.pode("Z", Atividade::Ler));
        assert!(u.pode("Z", Atividade::Inserir));
        assert!(!u.pode("Z", Atividade::Excluir), "nega por omissao");

        // Base listada vazia: nega tudo, sem cair no curinga.
        assert!(!u.pode("W", Atividade::Ler));

        // Base nao listada: cai no curinga.
        assert!(u.pode("QualquerOutra", Atividade::Ler));
        assert!(!u.pode("QualquerOutra", Atividade::Inserir));
    }

    #[test]
    fn sem_curinga_e_sem_base_nega_tudo() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}","bases":{{"Z":{{"ler":true}}}}}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        let u = c.por_login("ana").unwrap();
        assert!(u.pode("Z", Atividade::Ler));
        for a in Atividade::TODAS {
            assert!(
                !u.pode("W", a),
                "base nao listada deveria negar {}",
                a.nome()
            );
        }
    }

    #[test]
    fn supervisor_pode_tudo_em_toda_base() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"chefe","senha_hash":"{}","supervisor":true}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        let u = c.por_login("chefe").unwrap();
        for a in Atividade::TODAS {
            assert!(
                u.pode("QualquerBase", a),
                "supervisor deveria poder {}",
                a.nome()
            );
        }
    }

    #[test]
    fn root_e_supervisor_mesmo_dizendo_o_contrario() {
        let txt = format!(
            r#"{{"root":{{"login":"root","senha_hash":"{}","supervisor":false,"ativo":false}}}}"#,
            hash_rapido("raiz")
        );
        let c = cadastro(&txt);
        let r = c.root.as_ref().unwrap();
        assert!(r.supervisor);
        assert!(r.ativo);
        assert!(r.pode("Z", Atividade::Administrar));
    }

    #[test]
    fn usuario_inativo_nao_entra_nem_faz_nada() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"afastado","senha_hash":"{}",
                 "ativo":false,"supervisor":true}}]}}"#,
            hash_rapido("x")
        );
        let c = cadastro(&txt);
        assert!(c.autenticar("afastado", "x").is_none());
        let u = c.por_login("afastado").unwrap();
        assert!(!u.pode("Z", Atividade::Ler), "inativo nao pode nem ler");
    }

    #[test]
    fn autenticacao() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}"}}]}}"#,
            hash_rapido("Senha Certa")
        );
        let c = cadastro(&txt);
        assert!(c.autenticar("ana", "Senha Certa").is_some());
        assert!(c.autenticar("ana", "senha certa").is_none());
        assert!(c.autenticar("ana", "").is_none());
        assert!(c.autenticar("inexistente", "Senha Certa").is_none());
    }

    #[test]
    fn senha_em_texto_puro_funciona_mas_avisa() {
        let txt = r#"{"usuarios":[{"login":"legado","senha":"1234"}]}"#;
        let c = cadastro(txt);
        assert!(c.autenticar("legado", "1234").is_some());
        assert_eq!(c.avisos.len(), 1);
        assert!(
            c.avisos[0].contains("TEXTO PURO"),
            "aviso foi {:?}",
            c.avisos[0]
        );
        // E o que ficou em memoria ja e hash, nao a senha.
        assert!(senha::e_hash(&c.por_login("legado").unwrap().senha_hash));
    }

    #[test]
    fn a_ficha_nunca_devolve_a_senha() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}"}}]}}"#,
            hash_rapido("segredo")
        );
        let c = cadastro(&txt);
        let ficha = c.fichas().escrever();
        assert!(!ficha.contains("senha"), "a ficha vazou: {ficha}");
        assert!(!ficha.contains("pbkdf2"));
        assert!(ficha.contains("\"login\":\"ana\""));
    }

    #[test]
    fn cadastro_invalido_e_recusado() {
        let h = hash_rapido("x");
        for ruim in [
            r#"{"usuarios":[{"login":""}]}"#.to_string(),
            format!(
                r#"{{"usuarios":[{{"login":"a","senha_hash":"{h}"}},
                        {{"login":"a","senha_hash":"{h}"}}]}}"#
            ),
            format!(
                r#"{{"usuarios":[{{"id":5,"login":"a","senha_hash":"{h}"}},
                        {{"id":5,"login":"b","senha_hash":"{h}"}}]}}"#
            ),
            r#"{"usuarios":[{"login":"a","senha_hash":"nao-e-hash"}]}"#.to_string(),
            r#"{"usuarios":[{"login":"a"}]}"#.to_string(),
            format!(
                r#"{{"root":{{"login":"root","senha_hash":"{h}"}},
                        "usuarios":[{{"login":"root","senha_hash":"{h}"}}]}}"#
            ),
        ] {
            assert!(
                Cadastro::de_json(&Json::analisar(&ruim).unwrap()).is_err(),
                "deveria recusar: {ruim}"
            );
        }
    }

    #[test]
    fn id_sai_do_login_quando_omitido_e_e_estavel() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"adriano","senha_hash":"{}"}}]}}"#,
            hash_rapido("x")
        );
        let a = cadastro(&txt);
        let b = cadastro(&txt);
        let id = a.por_login("adriano").unwrap().id;
        assert_eq!(id, b.por_login("adriano").unwrap().id);
        assert!(id > 0, "o id vai para o .log e nao pode ser zero");
    }

    #[test]
    fn cada_operacao_exige_a_atividade_certa() {
        assert_eq!(Atividade::da_operacao("ping"), None);
        assert_eq!(Atividade::da_operacao("login"), None);
        assert_eq!(Atividade::da_operacao("buscar"), Some(Atividade::Ler));
        assert_eq!(Atividade::da_operacao("inserir"), Some(Atividade::Inserir));
        assert_eq!(
            Atividade::da_operacao("atualizar"),
            Some(Atividade::Alterar)
        );
        assert_eq!(Atividade::da_operacao("excluir"), Some(Atividade::Excluir));
        assert_eq!(Atividade::da_operacao("diario"), Some(Atividade::Diario));
        assert_eq!(Atividade::da_operacao("ips"), Some(Atividade::Administrar));
        // Operacao desconhecida exige o maior poder, em vez de passar batido.
        assert_eq!(
            Atividade::da_operacao("op_que_nao_existe"),
            Some(Atividade::Administrar)
        );
    }
}
