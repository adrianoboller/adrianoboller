# Add session, login and permission gates to server
# 27/08 19:04

import re
p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

s=s.replace('''use crate::acesso::{Acesso, LogAcessos};
use crate::config::Config;
use crate::valores::{json_para_chave, json_para_linha, linha_para_json};''','''use crate::acesso::{Acesso, LogAcessos};
use crate::config::Config;
use crate::usuarios::{Atividade, Usuario};
use crate::valores::{json_para_chave, json_para_linha, linha_para_json};''')

# --- Sessao ---
s=s.replace('''pub struct Servidor {''','''/// Estado de uma conexao.
///
/// A senha e conferida com PBKDF2, que custa da ordem de 100 ms de proposito.
/// Fazer isso a cada pedido inviabilizaria o servidor, entao a autenticacao
/// acontece UMA VEZ por conexao e o resultado fica aqui.
#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
}

impl Sessao {
    fn login(&self) -> &str {
        self.usuario.as_ref().map(|u| u.login.as_str()).unwrap_or("")
    }

    /// Id gravado no `.log` da tabela como autor da operacao.
    /// Zero quando a conexao veio pelo token de servico, sem login.
    fn id(&self) -> u32 {
        self.usuario.as_ref().map(|u| u.id).unwrap_or(0)
    }
}

pub struct Servidor {''')

# --- atender: cria a sessao ---
s=s.replace('''        let mut linha = String::new();
        loop {''','''        let mut sessao = Sessao::default();
        let mut linha = String::new();
        loop {''')
s=s.replace('''            let (op, autenticado, resultado) = self.despachar(&linha);''','''            let (op, autenticado, resultado) = self.despachar(&linha, &mut sessao);''')
s=s.replace('''            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                autenticado,''','''            self.anotar(&Acesso {
                quando_ms,
                ip: ip.clone(),
                porta_origem: porta,
                op: op.clone(),
                usuario: sessao.login().to_string(),
                autenticado,''')

# Os dois Acesso sem sessao (conexao recusada)
s=s.replace('''                                op: "conexao".into(),
                                autenticado: false,''','''                                op: "conexao".into(),
                                usuario: String::new(),
                                autenticado: false,''')
s=s.replace('''                op: "conexao".into(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),''','''                op: "conexao".into(),
                usuario: String::new(),
                autenticado: false,
                ok: false,
                duracao_ms: 0,
                erro: Some("ip fora da lista de permitidos".into()),''')

# --- despachar: token, login, permissao ---
s=s.replace('''    /// Le o pedido, confere o token e executa. Devolve (operacao, autenticado,
    /// resultado) para que o log registre mesmo o que falhou.
    fn despachar(&self, linha: &str) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() { "ping".to_string() } else { op };

        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            return (op, false, Err(PhxError::Esquema("token invalido".into())));
        }
        if self.config.somente_leitura && OPS_ESCRITA.contains(&op.as_str()) {
            return (
                op,
                true,
                Err(PhxError::Esquema(
                    "servidor em modo somente leitura".into(),
                )),
            );
        }
        let r = self.executar(&op, &pedido);
        (op, true, r)
    }''','''    /// Le o pedido e o leva por tres portoes, nesta ordem: o token (a rede),
    /// o login (a identidade) e a permissao (o poder). Devolve (operacao,
    /// autenticado, resultado) para que o log registre mesmo o que falhou.
    fn despachar(&self, linha: &str, sessao: &mut Sessao) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() { "ping".to_string() } else { op };

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            return (op, false, Err(PhxError::Esquema("token invalido".into())));
        }

        // Portao 2 -- o login.
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            return (op, r.is_ok(), r);
        }
        let exige_login = !self.config.cadastro.vazio();
        if exige_login && sessao.usuario.is_none() && Atividade::da_operacao(&op).is_some() {
            return (
                op,
                true,
                Err(PhxError::Esquema(
                    "faca login antes: {\\"op\\":\\"login\\",\\"usuario\\":...,\\"senha\\":...}".into(),
                )),
            );
        }

        if self.config.somente_leitura && OPS_ESCRITA.contains(&op.as_str()) {
            return (
                op,
                true,
                Err(PhxError::Esquema(
                    "servidor em modo somente leitura".into(),
                )),
            );
        }

        // Portao 3 -- o poder sobre a base deste pedido.
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(&op), sessao.usuario.as_ref())
        {
            let base = pedido.texto_ou("database", "");
            if !usuario.pode(base, atividade) {
                return (
                    op,
                    true,
                    Err(PhxError::Esquema(format!(
                        "{} nao tem permissao de {} em {}",
                        usuario.login,
                        atividade.nome(),
                        if base.is_empty() { "(sem base)" } else { base }
                    ))),
                );
            }
        }

        let r = self.executar(&op, &pedido, sessao);
        (op, true, r)
    }

    /// Confere login e senha e guarda a identidade na conexao.
    fn op_login(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        let login = p
            .texto_ou("usuario", p.texto_ou("login", ""))
            .trim()
            .to_string();
        let clara = p.texto_ou("senha", "");
        if login.is_empty() {
            return Err(PhxError::Esquema("informe \\"usuario\\" e \\"senha\\"".into()));
        }
        match self.config.cadastro.autenticar(&login, clara) {
            Some(u) => {
                let ficha = u.ficha();
                sessao.usuario = Some(u.clone());
                Ok(ficha)
            }
            None => {
                sessao.usuario = None;
                // Mensagem unica de proposito: nao dizer se o que errou foi o
                // login ou a senha.
                Err(PhxError::Esquema("usuario ou senha invalidos".into()))
            }
        }
    }''')

# --- executar ganha a sessao ---
s=s.replace('''    fn executar(&self, op: &str, p: &Json) -> Result<Json> {''','''    fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {''')
s=s.replace('''            "config" => Ok(self.config.para_json()),''','''            "config" => Ok(self.config.para_json()),
            "quem_sou" => Ok(match &sessao.usuario {
                Some(u) => u.ficha(),
                None => Json::objeto(vec![
                    ("usuario", Json::Nulo),
                    ("via", Json::texto_de("token de servico")),
                ]),
            }),
            "usuarios" => Ok(self.config.cadastro.fichas()),''')

# Ops que abrem tabela passam a receber a sessao
for op in ["esquema","ler","varrer","buscar","inserir","atualizar","excluir","diario","verificar","reindexar"]:
    s=s.replace(f'"{op}" => self.op_{op}(p),', f'"{op}" => self.op_{op}(p, sessao),')
    s=s.replace(f'fn op_{op}(&self, p: &Json) -> Result<Json> {{', f'fn op_{op}(&self, p: &Json, sessao: &Sessao) -> Result<Json> {{')
s=s.replace('self.abrir(p)?','self.abrir(p, sessao)?')

s=s.replace('''    fn abrir(&self, p: &Json) -> Result<Table> {''','''    fn abrir(&self, p: &Json, sessao: &Sessao) -> Result<Table> {''')
s=s.replace('''        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        dados.abrir_database(database)?.abrir_qualificada(tabela)
    }''','''        let dados = self.dados.lock().map_err(|_| trava_envenenada())?;
        let mut t = dados.abrir_database(database)?.abrir_qualificada(tabela)?;
        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        Ok(t)
    }''')

s=s.replace('''            ("ping", Json::texto_de(VERSAO)),''','''            ("ping", Json::texto_de(VERSAO)),''')
open(p,'w').write(s)
print("servidor.rs: sessao, login e permissoes")
