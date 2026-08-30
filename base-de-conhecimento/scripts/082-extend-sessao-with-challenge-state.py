# Extend Sessao with challenge state
# 27/08 19:44

p='crates/phxsql-server/src/http.rs'
s=open(p).read()

s = s.replace('''/// Uma sessao do navegador: quem entrou e ate quando vale.
#[derive(Debug, Clone)]
pub struct Sessao {
    pub login: String,
    pub expira_ms: i64,
}''','''/// Uma sessao do navegador: quem entrou e ate quando vale.
///
/// `login` vazio e uma sessao ainda anonima -- ela nasce assim no `desafio`,
/// que acontece ANTES de haver identidade, e so ganha nome quando o `login`
/// da certo. E o que permite o desafio-resposta por HTTP: o nonce precisa
/// sobreviver de um pedido para o outro, e a sessao e o unico lugar que
/// atravessa os dois.
#[derive(Debug, Clone)]
pub struct Sessao {
    pub login: String,
    pub expira_ms: i64,
    /// Desafio em aberto: (usuario, nonce do servidor, quando expira).
    pub desafio: Option<(String, String, i64)>,
}''')

s = s.replace('''            Sessao {
                login: login.to_string(),
                expira_ms: agora_ms + duracao_ms,
            },''','''            Sessao {
                login: login.to_string(),
                expira_ms: agora_ms + duracao_ms,
                desafio: None,
            },''')

s = s.replace('''    pub fn encerrar(&mut self, id: &str) -> bool {''','''    /// Amarra um login a uma sessao que ja existe. Falso se ela sumiu.
    pub fn definir_login(&mut self, id: &str, login: &str) -> bool {
        match self.dentro.get_mut(id) {
            Some(s) => {
                s.login = login.to_string();
                true
            }
            None => false,
        }
    }

    /// Guarda o desafio em aberto desta sessao.
    pub fn guardar_desafio(&mut self, id: &str, desafio: (String, String, i64)) {
        if let Some(s) = self.dentro.get_mut(id) {
            s.desafio = Some(desafio);
        }
    }

    /// Retira o desafio em aberto. Vale UMA vez: sai daqui e nao volta,
    /// dando certo ou errado -- igual ao caminho TCP.
    pub fn tomar_desafio(&mut self, id: &str) -> Option<(String, String, i64)> {
        self.dentro.get_mut(id).and_then(|s| s.desafio.take())
    }

    pub fn encerrar(&mut self, id: &str) -> bool {''')
open(p,'w').write(s)
print("http.rs sessoes ok")
