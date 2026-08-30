# Wire sessions and sair into servidor.rs
# 27/08 19:44

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# --- imports
s = s.replace('''use crate::config::Config;''','''use crate::config::Config;
use crate::http;''')

# --- campo sessoes
s = s.replace('''    lista_negra: Mutex<Blacklist>,
    conexoes: AtomicUsize,
}''','''    lista_negra: Mutex<Blacklist>,
    /// Sessoes do navegador. Vazio enquanto a interface web estiver desligada.
    sessoes: Mutex<http::Sessoes>,
    conexoes: AtomicUsize,
}''')

s = s.replace('''            lista_negra: Mutex::new(lista_negra),
            conexoes: AtomicUsize::new(0),''','''            lista_negra: Mutex::new(lista_negra),
            sessoes: Mutex::new(http::Sessoes::default()),
            conexoes: AtomicUsize::new(0),''')

# --- sobe a interface web antes do laco de aceitacao
s = s.replace('''        for conexao in ouvinte.incoming() {''','''        self.subir_web();

        for conexao in ouvinte.incoming() {''')

# --- sair, logo depois do login, dentro de despachar
s = s.replace('''        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()''','''        // Sair nao precisa de poder nenhum: e devolver o que se tem.
        if op == "sair" {
            sessao.usuario = None;
            sessao.desafio = None;
            return (op, true, Ok(Json::objeto(vec![("saiu", Json::Bool(true))])));
        }
        if !self.config.cadastro.vazio()
            && sessao.usuario.is_none()''')

open(p,'w').write(s)
print("ok")
