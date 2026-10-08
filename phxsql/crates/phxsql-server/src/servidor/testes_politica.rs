use super::*;

/// Um servidor em `somente_leitura` nao pode criar nem apagar tabela.
///
/// Este teste existe porque o furo ja aconteceu: as tres operacoes de
/// gestao de tabela entraram no despacho e ficaram DE FORA da lista de
/// escrita, e um servidor marcado somente-leitura teria aceitado apagar os
/// cinco arquivos de uma tabela. A lista e escrita a mao, entao quem
/// acrescentar uma operacao que grava precisa lembrar dela -- e este teste
/// e o lembrete.
#[test]
fn tudo_que_grava_esta_na_lista_de_escrita() {
    for op in [
        "inserir",
        "atualizar",
        "excluir",
        "reindexar",
        "criar_database",
        "criar_tabela",
        "excluir_tabela",
        "duplicar_tabela",
        "copiar_tabela",
        "ajustar_sequencia",
        "criar_sequencia",
        "excluir_sequencia",
        "proximo_da_sequencia",
        "dblink_salvar",
        "dblink_excluir",
        // Derrubar conexao alheia nao e leitura: um servidor somente
        // leitura nao deve poder interromper o trabalho de ninguem.
        "encerrar_sessao",
        // Restaurar desmarca a coluna de sistema, e esvaziar apaga a
        // lixeira inteira: os dois gravam. O esvaziar so mexe no no, e
        // isso e outra lista (`OPS_DO_NO`), e nao a ausencia desta.
        "restaurar",
        "esvaziar_lixeira",
        "expurgar_trilha",
        // Carga em lote grava, e grava muito.
        "inserir_lote",
        // Reservar a tabela e declarar que vai gravar.
        "bulkinsert",
    ] {
        assert!(
            OPS_ESCRITA.contains(&op),
            "{op:?} grava e nao esta em OPS_ESCRITA: \
                 um servidor somente-leitura aceitaria"
        );
    }
}

/// As que gravam SO no arquivo deste no: escrita para toda pergunta do
/// `OPS_ESCRITA`, e fora so da do somente-leitura/replica -- pedidos
/// 368/C2 e 499. A prova pelo soquete esta em `tests/lixeira-da-replica.rs`.
#[test]
fn o_que_grava_so_no_no_e_escrita_menos_para_a_replica() {
    for op in ["esvaziar_lixeira", "expurgar_trilha"] {
        assert!(OPS_ESCRITA.contains(&op), "{op:?} deixou de ser escrita");
        assert!(OPS_DO_NO.contains(&op), "{op:?} deixou de ser do no");
        assert!(!grava_dado_replicado(op), "{op:?}");
        assert_eq!(
            Atividade::da_operacao(op),
            Some(Atividade::Administrar),
            "{op:?} apaga sem volta e saiu do portao do administrador"
        );
    }
    // O controle: o resto da lista continua gravando dado replicado.
    assert!(grava_dado_replicado("inserir") && grava_dado_replicado("restaurar"));
    assert!(OPS_DO_NO.iter().all(|op| OPS_ESCRITA.contains(op)));
}

#[test]
fn o_que_so_le_fica_fora_da_lista() {
    for op in [
        "ping",
        "config",
        "bancos",
        "tabelas",
        "esquema",
        "ler",
        "varrer",
        "buscar",
        "diario",
        "verificar",
        "painel",
        "sistema",
        "acessos",
        "usuarios",
        "sistabelas",
        "siscolunas",
        "pivotar",
        "sequencias",
        "juntar",
        "unir",
        "estatisticas",
        "checksum",
        "exportar",
        "sessoes",
        "sistema",
        // Listar a lixeira e os motivos so le -- e e exatamente o que se
        // quer poder fazer num espelho somente-leitura, investigando.
        "lixeira",
        "motivos",
        // Conferir a carga nao grava nada -- e justamente o que se quer
        // poder fazer antes de decidir gravar.
        "importar_conferir",
        "dblink",
        "dblink_testar",
        "dblink_tabelas",
        "dblink_estrutura",
        "dblink_ler",
        // Nao esta na lista de proposito: por ela passa tanto consulta
        // quanto escrita, e a propria operacao confere qual e -- barrando
        // a escrita quando este servidor esta somente-leitura, mas
        // deixando a LEITURA funcionar num espelho.
        "dblink_consultar",
    ] {
        assert!(
            !OPS_ESCRITA.contains(&op),
            "{op:?} so le e esta na lista de escrita: \
                 um servidor somente-leitura recusaria sem motivo"
        );
    }
}
