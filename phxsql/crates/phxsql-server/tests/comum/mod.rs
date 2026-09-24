//! Apoio aos testes de INTEGRACAO do `phxsql-server` (cada arquivo de
//! `tests/` e um binario proprio, e nenhum deles enxerga o
//! `#[cfg(test)] mod apoio_teste` de dentro da lib).
//!
//! Pedido 150 -- a bateria nao limpava o que criava. O padrao velho era um
//! helper que devolvia so o `PathBuf`: sem guarda, o diretorio sobrevivia ao
//! teste, e sobrevivia especialmente quando o teste FALHAVA no meio, que e o
//! caso comum. `DirTemp` apaga no `Drop`, que o Rust roda tambem durante o
//! desenrolamento de um panic.
//!
//! O prefixo `phxsrv-it-` e proprio deste crate: com um prefixo por familia,
//! contar o que sobrou em `/tmp` diz de qual bateria veio o lixo.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Diretorio temporario exclusivo, removido no `Drop`.
///
/// `dead_code` permitido: cada arquivo de `tests/` inclui este modulo inteiro
/// por `mod comum;`, e o item que sobra sem uso muda de arquivo para arquivo.
#[allow(dead_code)]
pub struct DirTemp(pub PathBuf);

#[allow(dead_code)]
impl DirTemp {
    /// `rotulo` identifica o teste no nome do diretorio (so para depuracao);
    /// a unicidade real vem do PID do processo de teste e de um contador.
    pub fn novo(rotulo: &str) -> DirTemp {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("phxsrv-it-{}-{rotulo}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        DirTemp(p)
    }
}

impl Drop for DirTemp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Deref para `Path`: o teste que troca `PathBuf` por `DirTemp` continua
// escrevendo `&d`, `d.join(...)` e `d.display()` sem mexer em cada linha.
impl std::ops::Deref for DirTemp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

// `AsRef<Path>`: o Deref acima nao basta para `fn f(d: impl AsRef<Path>)` --
// a coercao de Deref so vale quando a assinatura pede `&Path` explicito.
impl AsRef<Path> for DirTemp {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Espera a porta REAL que um servidor ligou, sem nunca ter escolhido um
/// numero por fora -- pedido 401.
///
/// # O defeito que isto fecha
///
/// O padrao velho, copiado em ~25 arquivos de teste, era `porta_livre()`:
/// abre um `TcpListener` em `127.0.0.1:0`, LE a porta que o sistema deu,
/// SOLTA o ouvinte, e so entao escreve esse numero no `config.json` para o
/// servidor ligar ali minutos depois. Entre o soltar e o `bind` de verdade
/// ha uma janela -- e nessa janela, outro teste rodando em paralelo (o
/// `cargo test` sobe dezenas de binarios e threads ao mesmo tempo) pode
/// reservar o MESMO numero para si. Nao e "sortear melhor": enquanto existir
/// um instante entre escolher e ligar, a corrida existe.
///
/// # O conserto
///
/// Nao escolher nada: o `config.json` pede `"bind": "127.0.0.1:0"` (ou
/// `web.bind`/`rest.bind`/`rest.swagger_bind`), o SISTEMA OPERACIONAL escolhe
/// a porta no instante exato do `bind`, e so DEPOIS o teste pergunta ao
/// proprio servidor -- por `Servidor::porta_dos_dados()` e as tres irmas --
/// que numero foi esse. Nao ha janela para preencher porque nao ha escolha
/// alguma da parte do teste.
///
/// `obter` e chamado em laco curto ate devolver `Some`: a thread que roda
/// `escutar()` pode nao ter chegado ao `bind` no instante em que o chamador
/// pede a porta.
#[allow(dead_code)]
pub fn porta_real(mut obter: impl FnMut() -> Option<u16>) -> u16 {
    let ate = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(p) = obter() {
            return p;
        }
        assert!(
            std::time::Instant::now() < ate,
            "a porta nao ficou disponivel em 10 s -- o `bind` no `escutar()` \
             nunca aconteceu, ou o campo lido nao e o que este servidor usa"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// A mesma espera de `porta_real`, mas para quando a porta MUDA -- o
/// `servico_subir` troca a porta de dados em voo, e o numero novo so existe
/// depois da troca. Esperar so "Some" veria o numero VELHO e devolveria na
/// hora, antes de a troca acontecer.
#[allow(dead_code)]
pub fn porta_trocou_para(mut obter: impl FnMut() -> Option<u16>, anterior: u16) -> u16 {
    let ate = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(p) = obter() {
            if p != anterior {
                return p;
            }
        }
        assert!(
            std::time::Instant::now() < ate,
            "a porta continuou {anterior} por 10 s -- a troca nao aconteceu"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Uma porta que fica FECHADA de proposito -- o oposto de `porta_real`.
///
/// Existe para o punhado de cenarios em que o teste PRECISA que ninguem
/// atenda ali (um `noB` de cluster que nunca sobe, uma "porta ocupada" cujo
/// dono e outro `TcpListener` do proprio teste): nesses casos nao ha ouvinte
/// nenhum para perguntar depois, entao `porta_real` nao se aplica -- ler a
/// porta de volta so funciona quando ALGUEM liga nela de verdade.
///
/// A mesma janela teorica de `porta_livre()` continua aqui (reserva, solta,
/// e outro processo PODERIA pegar o numero antes do teste terminar) --
/// mas aqui isso e inerente ao que se quer medir, e nao um descuido: exigir
/// que a porta fique fechada durante o teste E pedir que nada a ocupe. Por
/// isso a funcao tem nome proprio, em vez de se chamar `porta_livre` como o
/// padrao que o pedido 401 fechou.
#[allow(dead_code)]
pub fn porta_fechada() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
