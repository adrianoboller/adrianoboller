//! A prova da fronteira, chamada como o C chamaria: ponteiros crus,
//! buffers do chamador e codigo de retorno.
//!
//! Nao ha atalho por dentro. Um teste que montasse a `Table` direto provaria
//! o motor, que ja tem os testes dele -- o que precisa de prova aqui e a
//! CAMADA: o panico que nao passa, o punho que se recusa depois de liberado,
//! o byte zero que sobrevive, o erro que fica na thread certa.

use std::ffi::CStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::valor::*;

// ------------------------------------------------------------------ auxilio

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Um diretorio so desta rodada, apagado no fim.
struct Area(PathBuf);

impl Area {
    fn nova(rotulo: &str) -> Area {
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let d =
            std::env::temp_dir().join(format!("phxsql-ffi-{}-{rotulo}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Area(d)
    }
    fn txt(&self) -> String {
        self.0.to_string_lossy().to_string()
    }
}

impl Drop for Area {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// O par `(ponteiro, tamanho)` de um `&str`, que e como tudo entra na ABI.
fn par(s: &str) -> (*const u8, usize) {
    (s.as_ptr(), s.len())
}

fn v_int(n: i64) -> PhxValor {
    PhxValor {
        tipo: PHX_INT,
        reservado: 0,
        numero: n,
        real: 0.0,
        dados: std::ptr::null(),
        tam: 0,
    }
}

fn v_bytes(tipo: i32, b: &[u8]) -> PhxValor {
    PhxValor {
        tipo,
        reservado: 0,
        numero: 0,
        real: 0.0,
        dados: b.as_ptr(),
        tam: b.len(),
    }
}

fn v_nulo() -> PhxValor {
    PhxValor {
        tipo: PHX_NULO,
        reservado: 0,
        numero: 0,
        real: 0.0,
        dados: std::ptr::null(),
        tam: 0,
    }
}

fn erro_agora() -> String {
    let mut buf = [0u8; 1024];
    let mut precisa = 0usize;
    unsafe { phx_ultimo_erro(buf.as_mut_ptr(), buf.len(), &mut precisa) };
    String::from_utf8_lossy(&buf[..precisa.min(buf.len())]).to_string()
}

/// Abre a base e cria a tabela de sempre: id (INT8, chave), nome (STR 40),
/// ficha (MEMO).
unsafe fn montar(area: &Area, tabela: &str) -> (*mut Punho<BaseFFI>, *mut Punho<TabelaFFI>) {
    let caminho = area.txt();
    let mut base: *mut Punho<BaseFFI> = std::ptr::null_mut();
    let (p, t) = par(&caminho);
    let (n, nt) = par("app");
    assert_eq!(phx_base_abrir(p, t, n, nt, PHX_CRIAR, &mut base), PHX_OK);

    let mut esq: *mut Punho<EsquemaFFI> = std::ptr::null_mut();
    let (p, t) = par(tabela);
    assert_eq!(phx_esquema_novo(p, t, &mut esq), PHX_OK);
    let (p, t) = par("id");
    assert_eq!(
        phx_esquema_coluna(esq, p, t, PHX_COL_INT8, 0, 0, 0, PHX_COL_OBRIGATORIA),
        PHX_OK
    );
    let (p, t) = par("nome");
    assert_eq!(
        phx_esquema_coluna(esq, p, t, PHX_COL_STR, 40, 0, 0, PHX_COL_OBRIGATORIA),
        PHX_OK
    );
    let (p, t) = par("ficha");
    assert_eq!(
        phx_esquema_coluna(esq, p, t, PHX_COL_MEMO, 0, 0, 0, 0),
        PHX_OK
    );
    let (p, t) = par("porId");
    assert_eq!(
        phx_esquema_indice(esq, p, t, PHX_IDX_UNICO | PHX_IDX_PRIMARIA),
        PHX_OK
    );
    assert_eq!(phx_esquema_indice_coluna(esq, 0, 0), PHX_OK);

    let mut tab: *mut Punho<TabelaFFI> = std::ptr::null_mut();
    let r = phx_tabela_criar(base, std::ptr::null(), 0, esq, &mut tab);
    assert_eq!(r, PHX_OK, "criar tabela: {}", erro_agora());
    assert_eq!(phx_esquema_liberar(esq), PHX_OK);
    (base, tab)
}

unsafe fn inserir(tab: *mut Punho<TabelaFFI>, id: i64, nome: &str) -> u64 {
    let linha = [v_int(id), v_bytes(PHX_TEXTO, nome.as_bytes()), v_nulo()];
    let mut rowid = 0u64;
    let r = phx_inserir(tab, linha.as_ptr(), linha.len(), &mut rowid);
    assert_eq!(r, PHX_OK, "inserir: {}", erro_agora());
    rowid
}

// ================================================================ o ciclo

#[test]
fn ciclo_basico_grava_le_e_varre() {
    unsafe {
        let area = Area::nova("ciclo");
        let (base, tab) = montar(&area, "clientes");

        let r1 = inserir(tab, 1, "Adriano Boller");
        let r2 = inserir(tab, 2, "Marcia Alves");
        assert_eq!((r1, r2), (1, 2), "o rowid e a ordem de digitacao");

        let mut qtd = 0u64;
        assert_eq!(
            phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
            PHX_OK
        );
        assert_eq!(qtd, 2);

        // Ler de volta, pela vista.
        let mut linha: *mut Punho<LinhaFFI> = std::ptr::null_mut();
        assert_eq!(phx_ler(tab, r1, &mut linha), PHX_OK);
        let mut vals: *const PhxValor = std::ptr::null();
        let mut n = 0usize;
        assert_eq!(phx_linha_valores(linha, &mut vals, &mut n), PHX_OK);
        let vista = std::slice::from_raw_parts(vals, n);
        assert_eq!(vista[0].tipo, PHX_INT);
        assert_eq!(vista[0].numero, 1);
        assert_eq!(vista[1].tipo, PHX_TEXTO);
        let nome = std::slice::from_raw_parts(vista[1].dados, vista[1].tam);
        assert_eq!(std::str::from_utf8(nome).unwrap(), "Adriano Boller");
        assert_eq!(phx_linha_liberar(linha), PHX_OK);

        // Varrer pelo cursor.
        let mut cur: *mut Punho<CursorFFI> = std::ptr::null_mut();
        assert_eq!(phx_cursor_abrir(tab, PHX_VISAO_ATIVAS, &mut cur), PHX_OK);
        let mut vistos = Vec::new();
        loop {
            let mut id = 0u64;
            match phx_cursor_proximo(tab, cur, &mut id) {
                PHX_OK => vistos.push(id),
                PHX_NAO_HA => break,
                outro => panic!("cursor devolveu {outro}: {}", erro_agora()),
            }
        }
        assert_eq!(vistos, vec![1, 2]);
        assert_eq!(phx_cursor_liberar(cur), PHX_OK);

        // Buscar pelo indice.
        let chave = [v_int(2)];
        let mut ids = [0u64; 4];
        let mut achados = 0usize;
        let (p, t) = par("porId");
        assert_eq!(
            phx_buscar(
                tab,
                p,
                t,
                chave.as_ptr(),
                1,
                ids.as_mut_ptr(),
                4,
                &mut achados
            ),
            PHX_OK
        );
        assert_eq!((achados, ids[0]), (1, r2));

        assert_eq!(phx_sincronizar(tab), PHX_OK);
        let mut rel = PhxRelatorio::default();
        assert_eq!(phx_verificar(tab, &mut rel), PHX_OK);
        assert_eq!(rel.registros, 2);

        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

#[test]
fn tabela_reabre_e_lista() {
    unsafe {
        let area = Area::nova("reabrir");
        let (base, tab) = montar(&area, "clientes");
        inserir(tab, 7, "Sete");
        assert_eq!(phx_sincronizar(tab), PHX_OK);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);

        let mut qtd = 0usize;
        assert_eq!(phx_base_tabelas_qtd(base, &mut qtd), PHX_OK);
        assert_eq!(qtd, 1);
        let mut buf = [0u8; 64];
        let mut precisa = 0usize;
        assert_eq!(
            phx_base_tabela_nome(base, 0, buf.as_mut_ptr(), buf.len(), &mut precisa),
            PHX_OK
        );
        assert_eq!(&buf[..precisa], b"clientes");

        let mut de_novo: *mut Punho<TabelaFFI> = std::ptr::null_mut();
        let (p, t) = par("clientes");
        assert_eq!(phx_tabela_abrir(base, p, t, &mut de_novo), PHX_OK);
        let mut n = 0u64;
        assert_eq!(
            phx_tabela_registros(de_novo, PHX_VISAO_ATIVAS, &mut n),
            PHX_OK
        );
        assert_eq!(n, 1);
        assert_eq!(phx_tabela_fechar(de_novo), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ============================================== o panico que nao atravessa

/// A garantia central desta camada. Se ela cair, o defeito nao e um teste
/// vermelho num aplicativo de celular: e o app fechando sozinho.
///
/// E o caminho e REAL, e nao um gatilho de teste: uma contagem absurda vinda
/// do C estoura a reserva do vetor dentro do `phx_inserir`.
#[test]
fn panico_nao_atravessa_a_fronteira() {
    unsafe {
        let area = Area::nova("panico");
        let (base, tab) = montar(&area, "clientes");

        let antes = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {})); // o panico e esperado; sem ruido
        let linha = [v_int(1)];
        let mut rowid = 0u64;
        let r = phx_inserir(tab, linha.as_ptr(), usize::MAX, &mut rowid);
        std::panic::set_hook(antes);

        assert_eq!(r, erro::PHX_ERRO_PANICO, "o panico tinha de virar codigo");
        assert!(
            erro_agora().contains("panico na fronteira"),
            "a mensagem tem de dizer o que houve: {}",
            erro_agora()
        );
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

/// Capturar o panico salva o processo e NAO conserta o objeto: o punho fica
/// envenenado e recusa trabalho ate ser reaberto.
#[test]
fn panico_envenena_o_punho_e_so_o_fechar_passa() {
    unsafe {
        let area = Area::nova("veneno");
        let (base, tab) = montar(&area, "clientes");

        let antes = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let linha = [v_int(1)];
        let mut rowid = 0u64;
        assert_eq!(
            phx_inserir(tab, linha.as_ptr(), usize::MAX, &mut rowid),
            erro::PHX_ERRO_PANICO
        );
        std::panic::set_hook(antes);

        // Uma chamada boa depois do panico tem de ser RECUSADA.
        let mut qtd = 0u64;
        assert_eq!(
            phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
            erro::PHX_ERRO_ENVENENADO
        );
        // E o fechar tem de continuar passando, senao panico viraria vazamento.
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

/// Nenhuma funcao exportada pode escapar da blindagem.
///
/// Ler o codigo nao pega isto: a funcao nova compila, passa nos testes dela e
/// so quebra no aplicativo do cliente. Entao o proprio fonte e conferido --
/// e a excecao unica esta nomeada aqui, com o motivo, para que acrescentar
/// uma segunda exija escrever por que.
#[test]
fn toda_funcao_exportada_e_blindada() {
    let fonte = include_str!("lib.rs");
    // `phx_erro_nome` devolve ponteiro para literal estatico: nao aloca, nao
    // toca em punho, nao ha o que entrar em panico -- e nao ha codigo de erro
    // que ela pudesse devolver.
    let isentas = ["phx_erro_nome"];

    let mut conferidas = 0;
    let mut faltando = Vec::new();
    let pedacos: Vec<&str> = fonte.split("extern \"C\" fn ").collect();
    for pedaco in pedacos.iter().skip(1) {
        let nome: String = pedaco
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if isentas.contains(&nome.as_str()) {
            continue;
        }
        // O corpo vai ate a proxima funcao exportada.
        let corpo = pedaco;
        let blindada = corpo.contains("blindado(")
            || corpo.contains("blindado_cru(")
            || corpo.contains("com(")
            || corpo.contains("liberar(");
        if !blindada {
            faltando.push(nome.clone());
        }
        conferidas += 1;
    }
    assert!(
        conferidas >= 40,
        "so achei {conferidas} funcoes exportadas -- a varredura quebrou"
    );
    assert!(
        faltando.is_empty(),
        "estas funcoes exportadas nao passam por blindado/com/liberar: {faltando:?}"
    );
}

// ============================================================ o byte zero

/// A razao de a ABI nao usar `NUL`-terminado. Quem trocar o par
/// `(ponteiro, tamanho)` por `strlen` derruba este teste -- e so este.
#[test]
fn byte_zero_no_dado_do_cliente_sobrevive() {
    unsafe {
        let area = Area::nova("nul");
        let (base, tab) = montar(&area, "clientes");

        let ficha = b"antes\0depois";
        let linha = [
            v_int(1),
            v_bytes(PHX_TEXTO, b"quem"),
            v_bytes(PHX_MEMO, ficha),
        ];
        let mut rowid = 0u64;
        assert_eq!(
            phx_inserir(tab, linha.as_ptr(), linha.len(), &mut rowid),
            PHX_OK,
            "{}",
            erro_agora()
        );

        let mut l: *mut Punho<LinhaFFI> = std::ptr::null_mut();
        assert_eq!(phx_ler(tab, rowid, &mut l), PHX_OK);
        let mut vals: *const PhxValor = std::ptr::null();
        let mut n = 0usize;
        assert_eq!(phx_linha_valores(l, &mut vals, &mut n), PHX_OK);
        let vista = std::slice::from_raw_parts(vals, n);
        let volta = std::slice::from_raw_parts(vista[2].dados, vista[2].tam);
        assert_eq!(
            volta, ficha,
            "o memo voltou truncado no primeiro byte zero -- alguem usou strlen"
        );
        assert_eq!(phx_linha_liberar(l), PHX_OK);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ================================================================ o erro

/// «Nao ha essa linha» tem de ser UMA resposta, e nao duas.
///
/// Achado pelo programa em C na primeira rodada dele, e nao lendo o codigo:
/// dentro do motor um slot livre devolve `Ok(None)` e um rowid alem do fim
/// devolve `NaoEncontrado`. A diferenca e real la dentro e invisivel para
/// quem chama -- e sem a dobra o aplicativo mostraria caixa vermelha para
/// metade dos «nao achei» e lista vazia para a outra metade.
#[test]
fn rowid_que_nao_existe_e_sempre_nao_ha_seja_qual_for_o_motivo() {
    unsafe {
        let area = Area::nova("naoha");
        let (base, tab) = montar(&area, "clientes");
        let r1 = inserir(tab, 1, "unico");

        // (a) alem do fim do arquivo
        let mut l: *mut Punho<LinhaFFI> = std::ptr::null_mut();
        assert_eq!(
            phx_ler(tab, 9999, &mut l),
            PHX_NAO_HA,
            "rowid alem do fim devolveu erro em vez de PHX_NAO_HA"
        );
        assert!(l.is_null());
        let mut v = 0u64;
        assert_eq!(phx_versao_da_linha(tab, 9999, &mut v), PHX_NAO_HA);

        // (b) slot que existiu e foi excluido de vez
        let (p, t) = par("saiu");
        let mut saiu = 0u8;
        assert_eq!(phx_excluir(tab, r1, p, t, &mut saiu), PHX_OK);
        assert_eq!(phx_ler(tab, r1, &mut l), PHX_NAO_HA);

        // E o 3001 continua doendo onde ele quer dizer outra coisa: indice
        // que nao existe e defeito de quem chamou.
        let chave = [v_int(1)];
        let mut ids = [0u64; 2];
        let mut achados = 0usize;
        let (p, t) = par("indiceQueNaoExiste");
        assert_eq!(
            phx_buscar(
                tab,
                p,
                t,
                chave.as_ptr(),
                1,
                ids.as_mut_ptr(),
                2,
                &mut achados
            ),
            3001,
            "indice inexistente nao pode virar PHX_NAO_HA"
        );

        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

/// Provoca um erro com uma marca reconhecivel na mensagem.
unsafe fn errar_com(marca: &str) {
    let mut base: *mut Punho<BaseFFI> = std::ptr::null_mut();
    let alvo = format!("nao-existe-{marca}");
    let (p, t) = par(&alvo);
    let (n, nt) = par(&alvo);
    // Abrir sem PHX_CRIAR um database que nao existe: erro garantido, e a
    // mensagem carrega o nome, que e a marca.
    assert_ne!(phx_base_abrir(p, t, n, nt, 0, &mut base), PHX_OK);
}

/// A vaga do ultimo erro e POR THREAD, e a ordem prova isso.
///
/// # Por que a barreira, e nao "a outra thread ve vazio"
///
/// A primeira versao deste teste olhava uma thread nova e exigia vaga vazia.
/// O executor de guardas devolveu NAO PEGOU: com uma vaga global o teste
/// PASSAVA -- porque os outros testes rodam em paralelo, e o `limpar()` de
/// qualquer um deles esvaziava a vaga global bem a tempo. Era um teste que
/// passava por engano, e a casa considera isso pior que teste que falta.
///
/// A ordem estrita conserta: A escreve, B escreve DEPOIS, e A tem de
/// continuar lendo o que A escreveu. Com vaga global, A le o de B.
#[test]
fn ultimo_erro_e_por_thread() {
    use std::sync::{Arc, Barrier};
    let a_escreveu = Arc::new(Barrier::new(2));
    let b_escreveu = Arc::new(Barrier::new(2));

    let (b1, b2) = (a_escreveu.clone(), b_escreveu.clone());
    let outra = std::thread::spawn(move || unsafe {
        b1.wait(); // espera A escrever
        errar_com("thread-B");
        b2.wait(); // avisa que B escreveu
    });

    unsafe { errar_com("thread-A") };
    a_escreveu.wait();
    b_escreveu.wait();
    let meu = erro_agora();
    outra.join().unwrap();

    assert!(
        meu.contains("thread-A"),
        "a vaga desta thread devia ter o erro DESTA thread, e trouxe: {meu:?}"
    );
    assert!(
        !meu.contains("thread-B"),
        "a mensagem da outra thread vazou para esta: {meu:?}"
    );
}

#[test]
fn buffer_pequeno_diz_quanto_falta_e_nao_trunca_calado() {
    unsafe {
        let mut precisa = 0usize;
        let r = phx_versao(std::ptr::null_mut(), 0, &mut precisa);
        assert_eq!(r, erro::PHX_ERRO_BUFFER);
        assert!(precisa > 0);

        let mut buf = vec![0u8; precisa + 1];
        let mut de_novo = 0usize;
        assert_eq!(
            phx_versao(buf.as_mut_ptr(), buf.len(), &mut de_novo),
            PHX_OK
        );
        assert_eq!(de_novo, precisa);
        assert_eq!(buf[precisa], 0, "o \\0 de cortesia tem de estar la");
    }
}

#[test]
fn o_codigo_do_motor_atravessa_intacto() {
    unsafe {
        let area = Area::nova("codigo");
        let (base, tab) = montar(&area, "clientes");
        inserir(tab, 1, "primeiro");
        // Mesma chave primaria: 3002 dos dois lados da fronteira.
        let linha = [v_int(1), v_bytes(PHX_TEXTO, b"segundo"), v_nulo()];
        let mut rowid = 0u64;
        let r = phx_inserir(tab, linha.as_ptr(), linha.len(), &mut rowid);
        assert_eq!(r, 3002, "chave duplicada e 3002 aqui e na porta de dados");
        let nome = CStr::from_ptr(phx_erro_nome(r) as *const std::os::raw::c_char);
        assert_eq!(nome.to_str().unwrap(), "DUPLICADO");
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

#[test]
fn texto_que_nao_e_utf8_e_recusado_e_nao_gravado() {
    unsafe {
        let area = Area::nova("utf8");
        let (base, tab) = montar(&area, "clientes");
        let torto = [0xffu8, 0xfe, 0xfd];
        let linha = [v_int(1), v_bytes(PHX_TEXTO, &torto), v_nulo()];
        let mut rowid = 0u64;
        assert_eq!(
            phx_inserir(tab, linha.as_ptr(), linha.len(), &mut rowid),
            erro::PHX_ERRO_UTF8
        );
        let mut qtd = 0u64;
        assert_eq!(
            phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
            PHX_OK
        );
        assert_eq!(qtd, 0, "recusa nao pode gravar meia linha");
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ================================================================ punhos

#[test]
fn punho_liberado_nao_volta_a_ser_usado() {
    unsafe {
        let area = Area::nova("etiqueta");
        let (base, tab) = montar(&area, "clientes");
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);

        let mut qtd = 0u64;
        assert_eq!(
            phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
            erro::PHX_ERRO_PONTEIRO,
            "punho ja liberado tinha de ser recusado pela etiqueta"
        );
        assert_eq!(
            phx_tabela_fechar(tab),
            erro::PHX_ERRO_PONTEIRO,
            "liberar duas vezes tinha de ser recusado"
        );
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

#[test]
fn punho_nulo_nao_derruba_nada() {
    unsafe {
        let mut qtd = 0u64;
        assert_eq!(
            phx_tabela_registros(std::ptr::null_mut(), PHX_VISAO_ATIVAS, &mut qtd),
            erro::PHX_ERRO_PONTEIRO
        );
        // Liberar nulo e o que `free(NULL)` faz: nada, sem reclamar.
        assert_eq!(phx_tabela_fechar(std::ptr::null_mut()), PHX_OK);
    }
}

// ================================================================ cursor

#[test]
fn cursor_de_outra_tabela_e_recusado_em_vez_de_apontar_para_lugar_nenhum() {
    unsafe {
        let area = Area::nova("cruzado");
        let (base, a) = montar(&area, "aaa");
        let (_, b) = montar(&area, "bbb");
        inserir(a, 1, "de a");
        inserir(b, 1, "de b");

        let mut cur: *mut Punho<CursorFFI> = std::ptr::null_mut();
        assert_eq!(phx_cursor_abrir(a, PHX_VISAO_ATIVAS, &mut cur), PHX_OK);
        let mut id = 0u64;
        assert_eq!(
            phx_cursor_proximo(b, cur, &mut id),
            erro::PHX_ERRO_USO,
            "o cursor de A nao pode andar sobre B"
        );
        assert_eq!(phx_cursor_proximo(a, cur, &mut id), PHX_OK);
        assert_eq!(phx_cursor_liberar(cur), PHX_OK);
        assert_eq!(phx_tabela_fechar(a), PHX_OK);
        assert_eq!(phx_tabela_fechar(b), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

/// O cursor busca em lotes; se ele parasse no fim do primeiro, uma tabela
/// grande seria entregue pela metade -- e sem erro nenhum.
#[test]
fn cursor_atravessa_a_fronteira_do_lote() {
    unsafe {
        let area = Area::nova("lote");
        let (base, tab) = montar(&area, "muitas");
        let total = LOTE_CURSOR * 2 + 7;
        for i in 1..=total {
            inserir(tab, i as i64, "x");
        }
        let mut cur: *mut Punho<CursorFFI> = std::ptr::null_mut();
        assert_eq!(phx_cursor_abrir(tab, PHX_VISAO_ATIVAS, &mut cur), PHX_OK);
        let mut n = 0u64;
        let mut ultimo = 0u64;
        loop {
            let mut id = 0u64;
            match phx_cursor_proximo(tab, cur, &mut id) {
                PHX_OK => {
                    assert!(id > ultimo, "o cursor voltou ou repetiu em {id}");
                    ultimo = id;
                    n += 1;
                }
                PHX_NAO_HA => break,
                outro => panic!("cursor devolveu {outro}"),
            }
        }
        assert_eq!(n, total, "o cursor parou na fronteira do lote");
        assert_eq!(phx_cursor_liberar(cur), PHX_OK);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

#[test]
fn cursor_de_indice_sai_na_ordem_do_indice() {
    unsafe {
        let area = Area::nova("indice");
        let (base, tab) = montar(&area, "clientes");
        inserir(tab, 30, "trinta");
        inserir(tab, 10, "dez");
        inserir(tab, 20, "vinte");

        let mut cur: *mut Punho<CursorFFI> = std::ptr::null_mut();
        let (p, t) = par("porId");
        assert_eq!(phx_cursor_abrir_indice(tab, p, t, &mut cur), PHX_OK);
        let mut ordem = Vec::new();
        loop {
            let mut id = 0u64;
            match phx_cursor_proximo(tab, cur, &mut id) {
                PHX_OK => ordem.push(id),
                PHX_NAO_HA => break,
                outro => panic!("cursor devolveu {outro}"),
            }
        }
        // rowid 2 tem id 10, rowid 3 tem id 20, rowid 1 tem id 30.
        assert_eq!(ordem, vec![2, 3, 1]);
        assert_eq!(phx_cursor_liberar(cur), PHX_OK);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ======================================================= janela de conflito

/// **O teste do comportamento VELHO.** Guarda nova entra pedida, nao imposta:
/// quem chama `phx_atualizar` sem versao continua gravando como sempre. Se
/// este cair, todo chamador escrito antes da janela parou de gravar.
#[test]
fn atualizar_sem_versao_continua_gravando_como_antes() {
    unsafe {
        let area = Area::nova("velho");
        let (base, tab) = montar(&area, "clientes");
        let rowid = inserir(tab, 1, "antes");
        // Outra sessao mexeu -- a versao subiu e ninguem avisou este chamador.
        let outro = [v_int(1), v_bytes(PHX_TEXTO, b"meio"), v_nulo()];
        assert_eq!(phx_atualizar(tab, rowid, outro.as_ptr(), 3), PHX_OK);

        let novo = [v_int(1), v_bytes(PHX_TEXTO, b"depois"), v_nulo()];
        assert_eq!(
            phx_atualizar(tab, rowid, novo.as_ptr(), 3),
            PHX_OK,
            "quem nao pede a guarda tem de continuar gravando"
        );
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

#[test]
fn atualizar_se_recusa_a_versao_velha_com_3004() {
    unsafe {
        let area = Area::nova("conflito");
        let (base, tab) = montar(&area, "clientes");
        let rowid = inserir(tab, 1, "antes");

        let mut versao = 0u64;
        assert_eq!(phx_versao_da_linha(tab, rowid, &mut versao), PHX_OK);

        // Outra sessao grava primeiro.
        let outro = [v_int(1), v_bytes(PHX_TEXTO, b"do outro"), v_nulo()];
        assert_eq!(phx_atualizar(tab, rowid, outro.as_ptr(), 3), PHX_OK);

        let meu = [v_int(1), v_bytes(PHX_TEXTO, b"o meu"), v_nulo()];
        assert_eq!(
            phx_atualizar_se(tab, rowid, meu.as_ptr(), 3, versao),
            3004,
            "gravar sobre versao velha tem de dar CONFLITO"
        );
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ================================================================ exclusao

#[test]
fn excluir_suave_some_da_varredura_e_restaurar_traz_de_volta() {
    unsafe {
        let area = Area::nova("suave");
        let (base, tab) = montar(&area, "clientes");
        let r1 = inserir(tab, 1, "fica");
        let r2 = inserir(tab, 2, "sai");

        let (p, t) = par("pedido do cliente");
        let mut saiu = 0u8;
        assert_eq!(phx_excluir_suave(tab, r2, p, t, &mut saiu), PHX_OK);
        assert_eq!(saiu, 1);

        let mut qtd = 0u64;
        assert_eq!(
            phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
            PHX_OK
        );
        assert_eq!(qtd, 1);

        let (p, t) = par("enganei-me");
        assert_eq!(phx_restaurar(tab, r2, p, t, &mut saiu), PHX_OK);
        assert_eq!(
            phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
            PHX_OK
        );
        assert_eq!(qtd, 2);

        // E o de vez sai mesmo, sem reaproveitar o slot.
        let (p, t) = par("de vez");
        assert_eq!(phx_excluir(tab, r2, p, t, &mut saiu), PHX_OK);
        let r3 = inserir(tab, 3, "novo");
        assert_eq!(r3, 3, "a ordem de digitacao e sagrada: slot nao se reusa");
        assert_eq!(r1, 1);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ============================================================= replicacao

/// Os ganchos ponta a ponta: A grava, o diario e lido com imagem, B aplica --
/// e os rowids saem iguais sem ninguem negociar nada.
#[test]
fn replicacao_de_ponta_a_ponta_pela_abi() {
    unsafe {
        let area_a = Area::nova("rep-a");
        let area_b = Area::nova("rep-b");
        let (base_a, a) = montar(&area_a, "clientes");
        let (base_b, b) = montar(&area_b, "clientes");

        assert_eq!(phx_imagem_no_diario(a, 1), PHX_OK);
        // Origem 7: e o que mata o laco infinito do bidirecional.
        assert_eq!(phx_forcar_proximo_evento(a, 0, 7), PHX_OK);
        inserir(a, 1, "Adriano");
        inserir(a, 2, "Marcia");

        let mut eventos = 0u64;
        assert_eq!(phx_diario_qtd(a, &mut eventos), PHX_OK);
        assert_eq!(eventos, 2);

        let mut lista = [PhxEvento::default(); 8];
        let mut lidos = 0usize;
        assert_eq!(
            phx_diario_ler(a, 0, lista.as_mut_ptr(), lista.len(), &mut lidos),
            PHX_OK
        );
        assert_eq!(lidos, 2);
        assert_eq!(lista[0].operacao, PHX_OP_INCLUSAO);
        assert_eq!(lista[0].rowid, 1);
        assert_eq!(lista[0].origem, 7, "a origem tem de atravessar");

        for i in 0..eventos {
            let mut ev = PhxEvento::default();
            let mut img: *mut Punho<ImagemFFI> = std::ptr::null_mut();
            assert_eq!(
                phx_diario_evento_com_imagem(a, i, &mut ev, &mut img),
                PHX_OK
            );
            assert!(!img.is_null(), "o evento {i} veio sem imagem");
            let mut dados: *const u8 = std::ptr::null();
            let mut tam = 0usize;
            assert_eq!(phx_imagem_bytes(img, &mut dados, &mut tam), PHX_OK);
            let mut saiu = 0u64;
            let r = phx_aplicar_evento(b, ev.operacao, ev.rowid, dados, tam, &mut saiu);
            assert_eq!(r, PHX_OK, "aplicar: {}", erro_agora());
            assert_eq!(saiu, ev.rowid, "o rowid da replica divergiu");
            assert_eq!(phx_imagem_liberar(img), PHX_OK);
        }

        let mut qtd = 0u64;
        assert_eq!(phx_tabela_registros(b, PHX_VISAO_ATIVAS, &mut qtd), PHX_OK);
        assert_eq!(qtd, 2);

        let mut l: *mut Punho<LinhaFFI> = std::ptr::null_mut();
        assert_eq!(phx_ler(b, 2, &mut l), PHX_OK);
        let mut vals: *const PhxValor = std::ptr::null();
        let mut n = 0usize;
        assert_eq!(phx_linha_valores(l, &mut vals, &mut n), PHX_OK);
        let vista = std::slice::from_raw_parts(vals, n);
        let nome = std::slice::from_raw_parts(vista[1].dados, vista[1].tam);
        assert_eq!(std::str::from_utf8(nome).unwrap(), "Marcia");
        assert_eq!(phx_linha_liberar(l), PHX_OK);

        assert_eq!(phx_tabela_fechar(a), PHX_OK);
        assert_eq!(phx_tabela_fechar(b), PHX_OK);
        assert_eq!(phx_base_fechar(base_a), PHX_OK);
        assert_eq!(phx_base_fechar(base_b), PHX_OK);
    }
}

// ================================================================= threads

/// O que a ABI PROMETE sobre thread, e nada alem: punhos diferentes em
/// threads diferentes, sobre tabelas diferentes.
#[test]
fn duas_threads_duas_tabelas() {
    let area = Area::nova("threads");
    let caminho = area.txt();
    let mut maos = Vec::new();
    for t in 0..2 {
        let caminho = caminho.clone();
        maos.push(std::thread::spawn(move || unsafe {
            let mut base: *mut Punho<BaseFFI> = std::ptr::null_mut();
            let (p, l) = par(&caminho);
            let db = format!("app{t}");
            let (n, nl) = par(&db);
            assert_eq!(phx_base_abrir(p, l, n, nl, PHX_CRIAR, &mut base), PHX_OK);

            let mut esq: *mut Punho<EsquemaFFI> = std::ptr::null_mut();
            let (p, l) = par("t");
            assert_eq!(phx_esquema_novo(p, l, &mut esq), PHX_OK);
            let (p, l) = par("id");
            assert_eq!(
                phx_esquema_coluna(esq, p, l, PHX_COL_INT8, 0, 0, 0, PHX_COL_OBRIGATORIA),
                PHX_OK
            );
            let mut tab: *mut Punho<TabelaFFI> = std::ptr::null_mut();
            assert_eq!(
                phx_tabela_criar(base, std::ptr::null(), 0, esq, &mut tab),
                PHX_OK
            );
            assert_eq!(phx_esquema_liberar(esq), PHX_OK);

            for i in 1..=200i64 {
                let linha = [v_int(i)];
                let mut rowid = 0u64;
                assert_eq!(phx_inserir(tab, linha.as_ptr(), 1, &mut rowid), PHX_OK);
            }
            let mut qtd = 0u64;
            assert_eq!(
                phx_tabela_registros(tab, PHX_VISAO_ATIVAS, &mut qtd),
                PHX_OK
            );
            assert_eq!(phx_tabela_fechar(tab), PHX_OK);
            assert_eq!(phx_base_fechar(base), PHX_OK);
            qtd
        }));
    }
    for m in maos {
        assert_eq!(m.join().unwrap(), 200);
    }
}

// ================================================================= valores

/// O `Decimal` e o unico valor que nao cabe em campo nenhum da struct: ele
/// viaja como 16 bytes, e os bytes precisam de um lugar estavel na volta.
#[test]
fn decimal_atravessa_nos_dezesseis_bytes() {
    unsafe {
        let area = Area::nova("decimal");
        let caminho = area.txt();
        let mut base: *mut Punho<BaseFFI> = std::ptr::null_mut();
        let (p, t) = par(&caminho);
        let (n, nt) = par("app");
        assert_eq!(phx_base_abrir(p, t, n, nt, PHX_CRIAR, &mut base), PHX_OK);

        let mut esq: *mut Punho<EsquemaFFI> = std::ptr::null_mut();
        let (p, t) = par("precos");
        assert_eq!(phx_esquema_novo(p, t, &mut esq), PHX_OK);
        let (p, t) = par("valor");
        assert_eq!(
            phx_esquema_coluna(esq, p, t, PHX_COL_DECIMAL, 0, 18, 2, 0),
            PHX_OK
        );
        let mut tab: *mut Punho<TabelaFFI> = std::ptr::null_mut();
        assert_eq!(
            phx_tabela_criar(base, std::ptr::null(), 0, esq, &mut tab),
            PHX_OK,
            "{}",
            erro_agora()
        );
        assert_eq!(phx_esquema_liberar(esq), PHX_OK);

        let escalado: i128 = 1234; // 12,34
        let bytes = escalado.to_le_bytes();
        let linha = [v_bytes(PHX_DECIMAL, &bytes)];
        let mut rowid = 0u64;
        assert_eq!(
            phx_inserir(tab, linha.as_ptr(), 1, &mut rowid),
            PHX_OK,
            "{}",
            erro_agora()
        );

        let mut l: *mut Punho<LinhaFFI> = std::ptr::null_mut();
        assert_eq!(phx_ler(tab, rowid, &mut l), PHX_OK);
        let mut vals: *const PhxValor = std::ptr::null();
        let mut n = 0usize;
        assert_eq!(phx_linha_valores(l, &mut vals, &mut n), PHX_OK);
        let vista = std::slice::from_raw_parts(vals, n);
        assert_eq!(vista[0].tipo, PHX_DECIMAL);
        assert_eq!(vista[0].tam, 16, "o decimal tem de vir nos 16 bytes");
        let volta = std::slice::from_raw_parts(vista[0].dados, 16);
        let mut a = [0u8; 16];
        a.copy_from_slice(volta);
        assert_eq!(i128::from_le_bytes(a), escalado);
        assert_eq!(phx_linha_liberar(l), PHX_OK);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

/// O `u64` acima de `i64::MAX` cabe nos mesmos 64 bits, e tem de voltar
/// exato -- quem trocar o campo por uma conversao com sinal derruba isto.
#[test]
fn uint_no_topo_da_faixa_volta_exato() {
    unsafe {
        let area = Area::nova("uint");
        let caminho = area.txt();
        let mut base: *mut Punho<BaseFFI> = std::ptr::null_mut();
        let (p, t) = par(&caminho);
        let (n, nt) = par("app");
        assert_eq!(phx_base_abrir(p, t, n, nt, PHX_CRIAR, &mut base), PHX_OK);

        let mut esq: *mut Punho<EsquemaFFI> = std::ptr::null_mut();
        let (p, t) = par("contadores");
        assert_eq!(phx_esquema_novo(p, t, &mut esq), PHX_OK);
        let (p, t) = par("quanto");
        assert_eq!(
            phx_esquema_coluna(esq, p, t, PHX_COL_UINT8, 0, 0, 0, 0),
            PHX_OK
        );
        let mut tab: *mut Punho<TabelaFFI> = std::ptr::null_mut();
        assert_eq!(
            phx_tabela_criar(base, std::ptr::null(), 0, esq, &mut tab),
            PHX_OK
        );
        assert_eq!(phx_esquema_liberar(esq), PHX_OK);

        let grande = u64::MAX - 3;
        let val = PhxValor {
            tipo: PHX_UINT,
            reservado: 0,
            numero: grande as i64,
            real: 0.0,
            dados: std::ptr::null(),
            tam: 0,
        };
        let mut rowid = 0u64;
        assert_eq!(
            phx_inserir(tab, &val, 1, &mut rowid),
            PHX_OK,
            "{}",
            erro_agora()
        );

        let mut l: *mut Punho<LinhaFFI> = std::ptr::null_mut();
        assert_eq!(phx_ler(tab, rowid, &mut l), PHX_OK);
        let mut vals: *const PhxValor = std::ptr::null();
        let mut n = 0usize;
        assert_eq!(phx_linha_valores(l, &mut vals, &mut n), PHX_OK);
        let vista = std::slice::from_raw_parts(vals, n);
        assert_eq!(vista[0].tipo, PHX_UINT);
        assert_eq!(vista[0].numero as u64, grande);
        assert_eq!(phx_linha_liberar(l), PHX_OK);
        assert_eq!(phx_tabela_fechar(tab), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

#[test]
fn tipo_desconhecido_recusa_na_hora() {
    unsafe {
        let area = Area::nova("tipo");
        let caminho = area.txt();
        let mut base: *mut Punho<BaseFFI> = std::ptr::null_mut();
        let (p, t) = par(&caminho);
        let (n, nt) = par("app");
        assert_eq!(phx_base_abrir(p, t, n, nt, PHX_CRIAR, &mut base), PHX_OK);
        let mut esq: *mut Punho<EsquemaFFI> = std::ptr::null_mut();
        let (p, t) = par("x");
        assert_eq!(phx_esquema_novo(p, t, &mut esq), PHX_OK);
        let (p, t) = par("c");
        assert_eq!(
            phx_esquema_coluna(esq, p, t, 999, 0, 0, 0, 0),
            erro::PHX_ERRO_USO
        );
        // E a coluna de texto sem largura tambem: um Str(0) nao guarda nada.
        assert_eq!(
            phx_esquema_coluna(esq, p, t, PHX_COL_STR, 0, 0, 0, 0),
            erro::PHX_ERRO_USO
        );
        assert_eq!(phx_esquema_liberar(esq), PHX_OK);
        assert_eq!(phx_base_fechar(base), PHX_OK);
    }
}

// ======================================================= o cabecalho de C

/// Extrai os nomes das funcoes exportadas do proprio fonte.
fn exportadas() -> Vec<String> {
    include_str!("lib.rs")
        .split("extern \"C\" fn ")
        .skip(1)
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .collect()
}

/// O cabecalho de C e escrito a mao, entao ele PODE envelhecer -- e envelhecer
/// calado e o defeito classico de FFI: a funcao existe no `.so`, nao existe no
/// `.h`, e ninguem descobre ate alguem precisar dela.
///
/// Os dois lados sao conferidos. So um lado deixaria passar a metade errada.
#[test]
fn o_cabecalho_de_c_e_a_biblioteca_declaram_as_mesmas_funcoes() {
    let h = include_str!("../include/phxsql.h");
    let nomes = exportadas();
    assert!(nomes.len() >= 40, "a varredura do fonte quebrou");

    let faltam_no_h: Vec<&String> = nomes
        .iter()
        .filter(|n| !h.contains(&format!("{n}(")))
        .collect();
    assert!(
        faltam_no_h.is_empty(),
        "exportadas e nao declaradas em phxsql.h: {faltam_no_h:?}"
    );

    // O outro lado: o cabecalho nao pode prometer o que nao existe. Os
    // `static inline` sao do proprio cabecalho e nao sao simbolos do binario.
    let mut sobrando = Vec::new();
    for linha in h.lines() {
        let l = linha.trim();
        if l.starts_with("static inline") || l.starts_with('*') || l.starts_with("/*") {
            continue;
        }
        if !(l.starts_with("int32_t phx_") || l.starts_with("const char *phx_")) {
            continue;
        }
        let inicio = l.find("phx_").unwrap();
        let nome: String = l[inicio..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !nomes.contains(&nome) {
            sobrando.push(nome);
        }
    }
    assert!(
        sobrando.is_empty(),
        "declaradas em phxsql.h e inexistentes na biblioteca: {sobrando:?}"
    );
}

/// Uma constante que diverge entre o Rust e o `.h` e pior que uma que falta:
/// o C compila, roda, e grava a coluna com o tipo errado.
#[test]
fn as_constantes_do_cabecalho_batem_com_as_do_rust() {
    let h = include_str!("../include/phxsql.h");
    // O `#define` do C: nome e valor, normalizados (sem `u`, sem parenteses).
    let mut no_h = std::collections::HashMap::new();
    for linha in h.lines() {
        let l = linha.trim();
        let Some(resto) = l.strip_prefix("#define ") else {
            continue;
        };
        let mut it = resto.split_whitespace();
        let (Some(nome), Some(valor)) = (it.next(), it.next()) else {
            continue;
        };
        if nome.contains('(') {
            continue; // macro com argumento, como PHX_T
        }
        let v = valor.trim_matches(|c| c == '(' || c == ')' || c == 'u');
        if let Ok(n) = v.parse::<i64>() {
            no_h.insert(nome.to_string(), n);
        }
    }
    assert!(no_h.len() > 50, "so li {} #define", no_h.len());

    let mut conferidas = 0;
    for fonte in [
        include_str!("lib.rs"),
        include_str!("valor.rs"),
        include_str!("erro.rs"),
    ] {
        for linha in fonte.lines() {
            let l = linha.trim();
            let Some(resto) = l.strip_prefix("pub const PHX_") else {
                continue;
            };
            let Some((esq, dir)) = resto.split_once(" = ") else {
                continue;
            };
            let nome = format!("PHX_{}", esq.split(':').next().unwrap().trim());
            let Ok(valor) = dir.trim_end_matches(';').trim().parse::<i64>() else {
                continue;
            };
            match no_h.get(&nome) {
                Some(&do_h) => {
                    assert_eq!(do_h, valor, "{nome} diverge entre o Rust e o phxsql.h");
                    conferidas += 1;
                }
                None => panic!("{nome} existe no Rust e nao no phxsql.h"),
            }
        }
    }
    assert!(conferidas > 50, "so conferi {conferidas} constantes");
}
