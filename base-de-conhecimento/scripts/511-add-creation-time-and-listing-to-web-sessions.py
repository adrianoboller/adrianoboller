# Add creation time and listing to web sessions
# 28/08 16:37

p='crates/phxsql-server/src/http.rs'
s=open(p).read()
a='''pub struct Sessao {
    pub login: String,
    pub expira_ms: i64,'''
b='''pub struct Sessao {
    pub login: String,
    pub expira_ms: i64,
    /// Quando a sessao comecou. So o prazo de expiracao nao responde "ha
    /// quanto tempo esta aberta", porque cada clique o renova.
    pub desde_ms: i64,'''
assert a in s; s=s.replace(a,b,1)
a='''            Sessao {
                login: login.to_string(),
                expira_ms: agora_ms + duracao_ms,
                desafio: None,
            },'''
b='''            Sessao {
                login: login.to_string(),
                expira_ms: agora_ms + duracao_ms,
                desde_ms: agora_ms,
                desafio: None,
            },'''
assert a in s; s=s.replace(a,b,1)
a='''    pub fn quantas(&self) -> usize {
        self.dentro.len()
    }'''
b='''    pub fn quantas(&self) -> usize {
        self.dentro.len()
    }

    /// As sessoes vivas, como (id, login, quando comecou, quando expira).
    ///
    /// O id sai CORTADO de proposito: ele e a credencial da sessao, e quem
    /// olha a lista de conexoes nao precisa de um cookie que da para colar
    /// noutro navegador. Oito letras bastam para achar a linha.
    pub fn listar(&self, agora_ms: i64) -> Vec<(String, String, i64, i64)> {
        let mut v: Vec<(String, String, i64, i64)> = self
            .dentro
            .iter()
            .filter(|(_, s)| s.expira_ms >= agora_ms)
            .map(|(id, s)| {
                (
                    id.chars().take(8).collect::<String>(),
                    s.login.clone(),
                    s.desde_ms,
                    s.expira_ms,
                )
            })
            .collect();
        v.sort_by_key(|x| x.2);
        v
    }

    /// Encerra pelo comeco do id, que e o que a lista mostra.
    pub fn encerrar_por_prefixo(&mut self, prefixo: &str) -> bool {
        let Some(id) = self
            .dentro
            .keys()
            .find(|k| k.starts_with(prefixo))
            .cloned()
        else {
            return false;
        };
        self.dentro.remove(&id);
        true
    }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
