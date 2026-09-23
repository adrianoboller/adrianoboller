//! Direito por coluna: o que cada operacao do protocolo faz com uma coluna
//! que o cadastro negou.
//!
//! # Por que a lista mora aqui, e por que ela e uma TABELA
//!
//! A petrea diz que o portao de permissao e UM so, e que **o campo que ele le
//! e o furo**: o portao geral confere `"tabela"`, e as operacoes que escondem
//! a tabela dele foram a porta dos fundos tres vezes. O direito por coluna
//! tem o mesmo formato de problema um nivel abaixo -- so que agora a pergunta
//! nao e «que tabela este pedido nomeia», e sim **«por onde este pedido
//! devolve dado de linha»**, que e uma pergunta que nao se responde por
//! padrao de nome nem por leitura casual.
//!
//! Entao ela e respondida uma vez, aqui, por operacao, em [`CLASSES`] -- e um
//! par de testes trava os dois sentidos do laco: operacao do catalogo que nao
//! esta classificada, e classificacao para operacao que nao existe. Chave
//! morta e pior que chave faltando: a primeira parece cobertura e nao cobre
//! nada.
//!
//! # As quatro classes, e por que a quarta e RECUSAR
//!
//! Uma peneira so sabe tirar a coluna de onde ela sabe procurar. O `exportar`
//! devolve um CSV, o `juntar` devolve colunas prefixadas de duas tabelas, o
//! `diario` devolve a imagem da linha, o `checksum` devolve um numero que
//! resume os bytes dela. Filtrar cada um desses seria escrever quatro
//! peneiras novas -- e a quinta, no dia em que entrasse uma operacao nova,
//! nasceria vazando calada. **Recusar e mais seguro que vazar**, e a recusa
//! diz o nome da operacao para quem a recebeu saber o que pedir no lugar.

use phxsql_core::json::Json;

/// Onde a resposta desta operacao carrega dado de linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Onde {
    /// A resposta INTEIRA e a linha -- ou ela vem no campo `linha`. E o `ler`,
    /// que responde uma coisa com `com_versao` e outra sem ele.
    Raiz,
    /// Uma lista de objetos no campo `linhas`, cada um com `rowid` e as
    /// colunas. E a forma do `varrer`, do `buscar` e do `procurar_texto`.
    Lista,
}

/// O que o direito por coluna faz com uma operacao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PorColuna {
    /// Nao devolve nem recebe dado de linha: nada a fazer.
    Nenhum,
    /// Devolve linha por um caminho que a peneira conhece.
    Le(Onde),
    /// Recebe colunas para gravar.
    Escreve,
    /// Descreve a ESTRUTURA. A coluna continua aparecendo -- estrutura nao e
    /// dado --, e a resposta ganha a lista do que aquele usuario nao le, para
    /// a tela nao pintar campo que nunca vai chegar.
    Estrutura,
    /// Devolve (ou grava) dado de linha por um caminho que a peneira NAO
    /// conhece. Recusa para quem tem regra de coluna na tabela.
    Recusa,
}

/// A classe de cada operacao do protocolo, incluindo os apelidos que o
/// `despachar` aceita -- porque o apelido chega ao portao como o nome chega, e
/// classificar so o canonico deixaria `pivot` passando onde `pivotar` recusa.
///
/// A leitura desta tabela so acontece para quem TEM regra de coluna: o
/// interruptor que decide isso e um `bool` da ficha da sessao, e vem antes.
pub const CLASSES: &[(&str, PorColuna)] = &[
    // ------------------------------------------------------------- a sessao
    ("ping", PorColuna::Nenhum),
    ("desafio", PorColuna::Nenhum),
    ("login", PorColuna::Nenhum),
    ("sair", PorColuna::Nenhum),
    ("quem_sou", PorColuna::Nenhum),
    ("catalogo", PorColuna::Nenhum),
    // -------------------------------------------------------- o que existe
    ("bancos", PorColuna::Nenhum),
    ("tabelas", PorColuna::Nenhum),
    // A estrutura NAO e o dado: esconder a coluna aqui faria a tela desenhar
    // um formulario que nao bate com a tabela, e faria o tradutor de SQL
    // planejar contra um esquema que nao existe. O que a resposta ganha e a
    // lista do que este usuario nao le.
    ("esquema", PorColuna::Estrutura),
    ("sistabelas", PorColuna::Nenhum),
    ("systables", PorColuna::Nenhum),
    ("siscolunas", PorColuna::Nenhum),
    ("syscolumns", PorColuna::Nenhum),
    // Mostra QUE a coluna guarda CPF, nunca o CPF -- e catalogo, como o
    // `siscolunas`.
    ("dados_pessoais", PorColuna::Nenhum),
    ("lgpd", PorColuna::Nenhum),
    ("sequencias", PorColuna::Nenhum),
    ("sequences", PorColuna::Nenhum),
    ("ajustar_sequencia", PorColuna::Nenhum),
    // ------------------------------------------------------------- leitura
    ("ler", PorColuna::Le(Onde::Raiz)),
    ("varrer", PorColuna::Le(Onde::Lista)),
    // O `coletar_rowids` peneira pelo MESMO `onde`/`expressao` do `varrer`, e
    // por isso a MESMA classe: enumerar quais rowids casam um valor de coluna
    // negada e' o mesmo vazamento que o `perguntar_pela_coluna_negada_recusa`
    // fecha no `varrer` -- classificar so' um deixaria o outro de porta aberta.
    ("coletar_rowids", PorColuna::Le(Onde::Lista)),
    ("buscar", PorColuna::Le(Onde::Lista)),
    ("procurar_texto", PorColuna::Le(Onde::Lista)),
    // A consulta em RAM devolve `linhas` com os mesmos campos do `varrer`, e
    // aceita o mesmo `onde` -- e a mesma peneira e a mesma recusa de
    // pergunta. Classificar so o caminho do disco deixaria a coluna negada
    // saindo inteira assim que alguem chamasse `memoria_carregar`.
    ("SelectMemory", PorColuna::Le(Onde::Lista)),
    ("selectmemory", PorColuna::Le(Onde::Lista)),
    ("selecionar_memoria", PorColuna::Le(Onde::Lista)),
    // A op `sql` nao devolve linha POR CONTA: cada passo dela sai pelo
    // `executar_derivado`, que passa por esta mesma funcao. Classificar
    // `sql` como leitura peneiraria duas vezes a mesma resposta -- e a
    // segunda peneira nao teria como saber de que tabela ela veio.
    ("sql", PorColuna::Nenhum),
    // O `consultar` e o mesmo argumento do `sql`, e por isso ele nao peneira:
    // cada sub-pedido dele sai pelo `executar_derivado` e paga la a peneira da
    // SUA tabela. Peneirar de novo aqui seria peneirar sem saber de que tabela
    // veio cada campo -- e depois de uma junção a linha tem campos de duas.
    ("consultar", PorColuna::Nenhum),
    // A visao guarda TEXTO, e nao linha: criar, listar e excluir nao devolvem
    // dado de tabela nenhuma. Quem le POR uma visao entra pela op `sql`, que
    // vira um `consultar`, e paga a peneira no sub-pedido -- que e onde a
    // tabela tem nome.
    ("criar_visao", PorColuna::Nenhum),
    ("visoes", PorColuna::Nenhum),
    ("excluir_visao", PorColuna::Nenhum),
    // ------------------------------ leitura por caminho que a peneira nao ve
    // Colunas prefixadas de duas tabelas, num objeto que a peneira nao sabe
    // desmontar sem saber de qual lado veio cada campo.
    ("juntar", PorColuna::Recusa),
    ("join", PorColuna::Recusa),
    ("unir", PorColuna::Recusa),
    ("union", PorColuna::Recusa),
    // O TERCEIRO IRMAO: a resposta traz a linha inteira dos dois lados dentro
    // de `diferentes`, e a lista `colunas` diz QUAIS mudaram -- entao ela
    // responde sobre a coluna negada mesmo que a peneira tirasse o valor.
    ("diferencas", PorColuna::Recusa),
    ("diff", PorColuna::Recusa),
    // Os ROTULOS das linhas do cruzamento SAO os valores da coluna.
    ("pivotar", PorColuna::Recusa),
    ("pivot", PorColuna::Recusa),
    // O `agrupar` e o mesmo argumento do pivot, com uma agravante: alem de os
    // rotulos dos grupos serem os valores da coluna, o AGREGADO fala dela sem
    // ela aparecer -- `{"funcao":"maximo","coluna":"salario"}` devolve o maior
    // salario num campo chamado `maximo_salario`, e a peneira, que procura
    // pelo NOME da coluna, nao acha nada para tirar. Peneirar por apelido
    // exigiria a peneira entender o pedido, e nao so a resposta.
    ("agrupar", PorColuna::Recusa),
    ("group_by", PorColuna::Recusa),
    // Um numero que resume os bytes da linha inteira. Nao mostra a coluna --
    // responde «e este valor?» a quem tentar, que e a mesma coisa devagar.
    ("checksum", PorColuna::Recusa),
    ("soma_de_verificacao", PorColuna::Recusa),
    // CSV, JSON ou SQL: texto, e nao estrutura.
    ("exportar", PorColuna::Recusa),
    ("export", PorColuna::Recusa),
    // A linha inteira, dentro de `descartadas[].linha`.
    ("lixeira", PorColuna::Recusa),
    ("trash", PorColuna::Recusa),
    // `antes` e `depois` de cada coluna marcada -- o valor, em texto.
    ("trilha", PorColuna::Recusa),
    ("trilha_lgpd", PorColuna::Recusa),
    // O QUARTO IRMAO do passado, e o mais discreto dos quatro: a resposta nao
    // traz a linha, traz `identidade` -- a chave primaria em TEXTO CLARO,
    // montada por `Table::identidade_de_valores`. Com o `cpf` como chave
    // primaria e marcado, um `motivos` devolvia o CPF de cada linha excluida
    // com data, hora e autor ao lado. Ele estava classificado pelo que o
    // pedido RECEBE; esta tabela responde por onde a RESPOSTA devolve dado de
    // linha, e sao coisas diferentes (pedido 343).
    ("motivos", PorColuna::Recusa),
    ("reasons", PorColuna::Recusa),
    // A imagem da linha, quando o diario a grava.
    ("diario", PorColuna::Recusa),
    // O fluxo de eventos com a imagem: e o `diario` pela porta da replicacao.
    ("replicar", PorColuna::Recusa),
    // Grava a imagem inteira vinda de fora, sem passar por `valores`.
    ("aplicar", PorColuna::Recusa),
    // Copiam a tabela para um nome NOVO -- e a regra de coluna e por nome de
    // tabela. Sem esta recusa, `duplicar_tabela` era «tire a restricao».
    ("duplicar_tabela", PorColuna::Recusa),
    ("copiar_tabela", PorColuna::Recusa),
    ("renomear_tabela", PorColuna::Recusa),
    // Leva os arquivos embora, com a coluna dentro deles.
    ("backup", PorColuna::Recusa),
    // Devolve o TEXTO dos pedidos capturados, e um `inserir` carrega valores.
    ("profiler", PorColuna::Recusa),
    // Le e grava tabela LOCAL, pelo cadastro da ligacao e nao pelo pedido.
    ("dblink_sincronizar", PorColuna::Recusa),
    // A carga colada nao e analisada aqui, e recortar texto para achar coluna
    // e exatamente o que a petrea proibe.
    ("importar_conferir", PorColuna::Recusa),
    // ------------------------------------------------------------- escrita
    ("inserir", PorColuna::Escreve),
    ("inserir_lote", PorColuna::Escreve),
    ("importar", PorColuna::Escreve),
    ("carga", PorColuna::Escreve),
    ("atualizar", PorColuna::Escreve),
    // `excluir` e `restaurar` mexem na LINHA inteira, e quem manda nisso e o
    // direito de excluir da tabela. Nao ha coluna no pedido -- e a resposta
    // dos dois e `rowid` mais um booleano, que e a metade que de fato sustenta
    // o `Nenhum`. O `motivos` morava aqui por vizinhanca e RESPONDE a
    // identidade da linha: ele mudou para o bloco da recusa (pedido 343).
    ("excluir", PorColuna::Nenhum),
    ("restaurar", PorColuna::Nenhum),
    ("marcar_lgpd", PorColuna::Nenhum),
    ("marcar_dado_pessoal", PorColuna::Nenhum),
    ("esvaziar_lixeira", PorColuna::Nenhum),
    ("bulkinsert", PorColuna::Nenhum),
    ("cargas", PorColuna::Nenhum),
    // ------------------------------------------------------------ transacao
    ("begin", PorColuna::Nenhum),
    ("start_transaction", PorColuna::Nenhum),
    ("begin_transaction", PorColuna::Nenhum),
    ("commit", PorColuna::Nenhum),
    ("rollback", PorColuna::Nenhum),
    ("savepoint", PorColuna::Nenhum),
    ("rollback_para", PorColuna::Nenhum),
    ("rollback_to_savepoint", PorColuna::Nenhum),
    ("release_savepoint", PorColuna::Nenhum),
    ("transacao", PorColuna::Nenhum),
    ("transacoes", PorColuna::Nenhum),
    // ------------------------------------------------------------- modelagem
    ("criar_database", PorColuna::Nenhum),
    ("criar_schema", PorColuna::Nenhum),
    ("criar_tabela", PorColuna::Nenhum),
    ("declarar_fk", PorColuna::Nenhum),
    ("excluir_fk", PorColuna::Nenhum),
    ("acrescentar_coluna", PorColuna::Nenhum),
    ("excluir_tabela", PorColuna::Nenhum),
    ("reindexar", PorColuna::Nenhum),
    ("verificar", PorColuna::Nenhum),
    ("reparar", PorColuna::Nenhum),
    // -------------------------------------------------------------- memoria
    ("memoria_carregar", PorColuna::Nenhum),
    ("memoria_liberar", PorColuna::Nenhum),
    ("memoria", PorColuna::Nenhum),
    // ----------------------------------------------------------- replicacao
    ("posicao", PorColuna::Nenhum),
    ("replicacao_estado", PorColuna::Nenhum),
    ("replicacao_testar", PorColuna::Nenhum),
    ("replicacao_ligar", PorColuna::Nenhum),
    ("replicacao_pular", PorColuna::Nenhum),
    ("spare_promover", PorColuna::Nenhum),
    ("cluster_pulso", PorColuna::Nenhum),
    ("cluster_estado", PorColuna::Nenhum),
    ("cluster_no_acrescentar", PorColuna::Nenhum),
    ("cluster_no_remover", PorColuna::Nenhum),
    // -------------------------------------------------------- administracao
    ("config", PorColuna::Nenhum),
    ("config_gravar", PorColuna::Nenhum),
    ("diretivas", PorColuna::Nenhum),
    ("diretiva_gravar", PorColuna::Nenhum),
    ("usuarios", PorColuna::Nenhum),
    ("usuario_criar", PorColuna::Nenhum),
    ("usuario_alterar", PorColuna::Nenhum),
    ("usuario_excluir", PorColuna::Nenhum),
    ("acessos", PorColuna::Nenhum),
    ("ips", PorColuna::Nenhum),
    ("bloqueios", PorColuna::Nenhum),
    ("desbloquear", PorColuna::Nenhum),
    ("bloqueios_exportar", PorColuna::Nenhum),
    ("whitelist_salvar", PorColuna::Nenhum),
    ("mensagens", PorColuna::Nenhum),
    ("mensagens_semear", PorColuna::Nenhum),
    ("idiomas", PorColuna::Nenhum),
    ("idiomas_carga", PorColuna::Nenhum),
    ("idiomas_padrao", PorColuna::Nenhum),
    ("idiomas_exportar", PorColuna::Nenhum),
    ("idiomas_importar", PorColuna::Nenhum),
    ("estatisticas", PorColuna::Nenhum),
    ("estatisticas_uso", PorColuna::Nenhum),
    ("sessoes", PorColuna::Nenhum),
    ("processlist", PorColuna::Nenhum),
    ("encerrar_sessao", PorColuna::Nenhum),
    ("kill", PorColuna::Nenhum),
    ("telemetria", PorColuna::Nenhum),
    ("telemetria_ligar", PorColuna::Nenhum),
    ("telemetria_desligar", PorColuna::Nenhum),
    ("telemetria_encerrar", PorColuna::Nenhum),
    ("painel", PorColuna::Nenhum),
    // A saude do disco nao le linha nenhuma: e o canario e os contadores.
    ("saude_disco", PorColuna::Nenhum),
    ("sistema", PorColuna::Nenhum),
    ("servico", PorColuna::Nenhum),
    ("servico_parar", PorColuna::Nenhum),
    ("servico_subir", PorColuna::Nenhum),
    ("conferir_backup", PorColuna::Nenhum),
    ("backups", PorColuna::Nenhum),
    // Restaurar poe arquivo de volta; nao mostra coluna nenhuma a ninguem, e
    // recusar aqui tiraria a recuperacao de desastre de quem tem uma regra de
    // coluna em qualquer tabela do servidor.
    ("restaurar_backup", PorColuna::Nenhum),
    ("jobs", PorColuna::Nenhum),
    ("job_listar", PorColuna::Nenhum),
    ("job_salvar", PorColuna::Nenhum),
    ("job_ligar", PorColuna::Nenhum),
    ("job_excluir", PorColuna::Nenhum),
    // O job roda pelo `executar_job`, que passa por esta mesma funcao com o
    // usuario DELE -- entao o pedido de dentro e classificado por conta.
    ("job_rodar", PorColuna::Nenhum),
    ("profiler_ligar", PorColuna::Nenhum),
    ("profiler_desligar", PorColuna::Nenhum),
    ("profiler_limpar", PorColuna::Nenhum),
    // ---------------------------------------------------------------- dblink
    // Estas leem o banco do OUTRO lado, onde o direito por coluna daqui nao
    // vale nem faz sentido -- quem manda la e o cadastro de la.
    ("dblink", PorColuna::Nenhum),
    ("dblink_salvar", PorColuna::Nenhum),
    ("dblink_excluir", PorColuna::Nenhum),
    ("dblink_testar", PorColuna::Nenhum),
    ("dblink_bancos", PorColuna::Nenhum),
    ("dblink_tabelas", PorColuna::Nenhum),
    ("dblink_estrutura", PorColuna::Nenhum),
    ("dblink_ler", PorColuna::Nenhum),
    ("dblink_consultar", PorColuna::Nenhum),
    ("dblink_ligar", PorColuna::Nenhum),
];

/// O que pedir NO LUGAR, quando a recusa acontece.
///
/// # Por que a frase nao pode ser uma so
///
/// Ela era: «Peca as colunas por ler, varrer, buscar ou SELECT». Para o
/// `juntar` e o `exportar` isso e um conselho -- os dois devolvem linha, e as
/// tres operacoes citadas devolvem linha com a peneira ligada. Para o
/// `agrupar` e uma frase sem sentido: quem pediu `{"funcao":"maximo",
/// "coluna":"salario"}` nao quer a linha, quer o maior salario, e varrer nao
/// lhe da isso. Para o `backup` e pior ainda -- ele nao devolve coluna
/// nenhuma, leva arquivo.
///
/// Conselho que nao serve gasta a confianca da mensagem inteira: quem o segue
/// uma vez e nao chega a lugar nenhum para de ler as outras.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saida {
    /// A operacao devolve LINHA por um caminho torto. Ha caminho reto.
    PelaLinha,
    /// A operacao devolve um RESUMO calculado sobre a coluna negada. Nao ha
    /// como calcula-lo sem le-la -- o caminho reto nao existe.
    Resumo,
    /// A operacao devolve o PASSADO da linha, em imagem. `ler`/`varrer` falam
    /// do agora e nao substituem nenhuma delas.
    Historico,
    /// A operacao leva a TABELA inteira embora (ou a copia). Nao ha coluna a
    /// pedir: ha um direito a ter.
    TabelaInteira,
}

impl Saida {
    pub fn texto(self) -> &'static str {
        match self {
            Saida::PelaLinha => {
                "Peca as colunas por ler, varrer, buscar ou SELECT, que aplicam o \
                 direito por coluna"
            }
            Saida::Resumo => {
                "o resumo se calcula SOBRE a coluna negada, entao nao ha como pedi-lo \
                 sem ela: agregue pelo que voce le, ou peca o direito a coluna"
            }
            Saida::Historico => {
                "o historico guarda a imagem da linha inteira, e imagem nao se \
                 peneira: peca o direito a coluna, ou peca a quem ja o tem"
            }
            Saida::TabelaInteira => {
                "esta operacao leva a tabela inteira, com a coluna dentro: nao ha \
                 coluna a pedir, ha direito a ter"
            }
        }
    }
}

/// A saida de cada operacao RECUSADA -- a lista e a mesma de [`CLASSES`], e um
/// par de testes trava os dois sentidos do laco, como la.
pub const SAIDAS: &[(&str, Saida)] = &[
    ("juntar", Saida::PelaLinha),
    ("join", Saida::PelaLinha),
    ("unir", Saida::PelaLinha),
    ("union", Saida::PelaLinha),
    ("diferencas", Saida::PelaLinha),
    ("diff", Saida::PelaLinha),
    ("pivotar", Saida::PelaLinha),
    ("pivot", Saida::PelaLinha),
    ("exportar", Saida::PelaLinha),
    ("export", Saida::PelaLinha),
    // O que deu nome ao O5 do pedido 245: o agregado FALA da coluna sem
    // mostra-la, e por isso «peca as colunas» nunca foi resposta aqui.
    ("agrupar", Saida::Resumo),
    ("group_by", Saida::Resumo),
    ("checksum", Saida::Resumo),
    ("soma_de_verificacao", Saida::Resumo),
    ("lixeira", Saida::Historico),
    ("trash", Saida::Historico),
    ("trilha", Saida::Historico),
    ("trilha_lgpd", Saida::Historico),
    // `ler`/`varrer` falam do agora e nao dizem por que uma linha saiu: nao
    // ha caminho reto para pedir no lugar, ha o direito a coluna.
    ("motivos", Saida::Historico),
    ("reasons", Saida::Historico),
    ("diario", Saida::Historico),
    ("replicar", Saida::Historico),
    ("profiler", Saida::Historico),
    ("aplicar", Saida::TabelaInteira),
    ("duplicar_tabela", Saida::TabelaInteira),
    ("copiar_tabela", Saida::TabelaInteira),
    ("renomear_tabela", Saida::TabelaInteira),
    ("backup", Saida::TabelaInteira),
    ("dblink_sincronizar", Saida::TabelaInteira),
    ("importar_conferir", Saida::TabelaInteira),
];

/// O que pedir no lugar desta operacao.
///
/// O padrao e [`Saida::TabelaInteira`] pelo mesmo motivo de `classe` nascer
/// recusando: operacao nova que ninguem classificou tem de cair no conselho
/// MAIS conservador -- «peca o direito» nunca e um conselho errado, e «peca as
/// colunas por varrer» seria, se a operacao nova nao devolver linha.
pub fn saida(op: &str) -> Saida {
    SAIDAS
        .iter()
        .find(|(n, _)| *n == op)
        .map(|(_, s)| *s)
        .unwrap_or(Saida::TabelaInteira)
}

/// A classe desta operacao.
///
/// Operacao fora da tabela nasce RECUSADA para quem tem regra de coluna. Isso
/// nao acontece hoje -- o par de testes prova que a lista e o catalogo sao a
/// mesma lista --, e o padrao existe para o dia em que alguem acrescentar uma
/// operacao e esquecer desta lista: ela nasce apertada demais e alguem
/// reclama, em vez de nascer vazando e ninguem perceber.
pub fn classe(op: &str) -> PorColuna {
    CLASSES
        .iter()
        .find(|(n, _)| *n == op)
        .map(|(_, c)| *c)
        .unwrap_or(PorColuna::Recusa)
}

/// As tabelas que este pedido nomeia -- inclusive as que o portao geral nao ve.
///
/// E a mesma varredura que a petrea manda fazer quando o portao passa a olhar
/// um campo novo: **procure quem NAO tem esse campo**. `juntar` guarda as duas
/// em `a.tabela` e `b.tabela`; `unir` guarda numa lista de nomes -- ou, com
/// `"partes"`, dentro de um PEDIDO por braco; `pivotar` poe a de
/// fatos no campo de sempre e as de consulta dentro de um `juntar` aninhado; e
/// as tres copias de tabela nomeiam o `destino`, que e onde a coluna negada
/// iria parar sem regra nenhuma.
///
/// Lista VAZIA nao quer dizer «nao toca em tabela»: quer dizer «nao da para
/// saber qual», e quem chama trata as duas coisas de forma diferente.
pub fn tabelas_do_pedido(op: &str, p: &Json) -> Vec<String> {
    let mut alvos: Vec<String> = Vec::new();
    let mut junte = |t: &str| {
        let t = t.trim();
        if !t.is_empty() && !alvos.iter().any(|a| a == t) {
            alvos.push(t.to_string());
        }
    };
    junte(p.texto_ou("tabela", ""));
    match op {
        "juntar" | "join" => {
            for lado in ["a", "b"] {
                if let Some(j) = p.campo(lado) {
                    junte(j.texto_ou("tabela", ""));
                }
            }
        }
        // O terceiro irmao: as duas tabelas moram em `"a"` e `"b"`, e nao
        // dentro de um objeto como no `juntar` -- por isso ele tem ramo
        // proprio, e nao cabe no de cima.
        "diferencas" | "diff" => {
            for lado in ["a", "b"] {
                junte(p.texto_ou(lado, ""));
            }
        }
        "unir" | "union" => {
            for x in p.campo("tabelas").and_then(Json::lista).unwrap_or(&[]) {
                match x.texto() {
                    Some(t) => junte(t),
                    None => junte(x.texto_ou("tabela", "")),
                }
            }
            // O braco que e PEDIDO (pedido 393) esconde a tabela um nivel
            // mais fundo -- e pode ser um `consultar`, que a esconde em mais
            // um. A varredura desce pelo mesmo motivo que desce no
            // `consultar`: lista que para no primeiro nivel e o inventario
            // que deixa de valer no dia em que alguem o usar como inventario.
            //
            // Isto NAO e o portao: cada braco paga o dele no
            // `executar_derivado`. E a resposta a pergunta «que tabelas este
            // pedido alcanca?», e ela ficaria vazia -- dizendo «nao da para
            // saber qual» sobre um pedido que nomeia as duas.
            for x in p.campo("partes").and_then(Json::lista).unwrap_or(&[]) {
                let op = x.texto_ou("op", "varrer").trim().to_string();
                for t in tabelas_do_pedido(&op, x) {
                    junte(&t);
                }
            }
        }
        "pivotar" | "pivot" => {
            for j in p.campo("juntar").and_then(Json::lista).unwrap_or(&[]) {
                junte(j.texto_ou("tabela", ""));
            }
        }
        "duplicar_tabela" | "copiar_tabela" | "renomear_tabela" => {
            junte(p.texto_ou("destino", ""));
        }
        // O `consultar` nao nomeia tabela em campo nenhum seu: ela mora nos
        // SUB-PEDIDOS -- `de`, `juntar[].de`, `escalar[].de`, `em[].de` e
        // `existe[].de` --, e cada um deles pode ser outro `consultar`. A
        // varredura desce, porque a lista que parasse no primeiro nivel
        // seria o inventario que deixa de valer no dia em que alguem a usar
        // como inventario. (A peneira nao passa por aqui para o `consultar`:
        // cada sub-pedido paga a dele no `executar_derivado`. Esta lista
        // existe para quem PERGUNTA que tabelas um pedido alcanca.)
        "consultar" => {
            let mut subs: Vec<&Json> = Vec::new();
            subs.extend(p.campo("de"));
            for lista in ["juntar", "escalar", "em", "existe"] {
                for item in p.campo(lista).and_then(Json::lista).unwrap_or(&[]) {
                    subs.extend(item.campo("de"));
                }
            }
            for sub in subs {
                let op = sub.texto_ou("op", "varrer").trim().to_string();
                for t in tabelas_do_pedido(&op, sub) {
                    junte(&t);
                }
            }
        }
        _ => {}
    }
    alvos
}

/// As colunas que o filtro `onde` do `varrer` nomeia.
///
/// Filtrar por uma coluna que nao se pode ler devolve a CONTAGEM das linhas
/// que casam -- e vinte perguntas dessas dizem o salario sem ele nunca ter
/// aparecido na resposta. A peneira sozinha nao fecha isso: ela tira o valor
/// DEPOIS de o filtro ja ter respondido.
pub fn colunas_do_onde(p: &Json) -> Vec<String> {
    p.campo("onde")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|f| f.texto_ou("coluna", "").trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}

/// O nome bate com o da regra? Sem caixa, e aparado.
pub fn mesmo_nome(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Tira as colunas negadas de um objeto que E uma linha.
fn peneirar_linha(linha: Json, negadas: &[String]) -> Json {
    match linha {
        Json::Objeto(pares) => Json::Objeto(
            pares
                .into_iter()
                .filter(|(k, _)| !negadas.iter().any(|n| mesmo_nome(n, k)))
                .collect(),
        ),
        outro => outro,
    }
}

/// Tira as colunas negadas da resposta, onde quer que a linha esteja.
pub fn peneirar(resposta: Json, onde: Onde, negadas: &[String]) -> Json {
    if negadas.is_empty() {
        return resposta;
    }
    match onde {
        // O `ler` responde a linha crua, ou `{rowid, linha, versao}` quando
        // pedem a versao. Sao duas formas da MESMA resposta, e por isso as
        // duas moram na mesma classe: separa-las seria uma classe que depende
        // de um campo do pedido, e a que alguem esquecesse vazaria.
        // O envelope do `com_versao` e reconhecido pelos TRES campos, e nao
        // so pelo `linha`: uma tabela pode ter uma coluna chamada `linha`, e
        // ai olhar um campo so faria a peneira descer dentro do DADO em vez
        // de dentro do envelope -- e devolver a coluna negada intacta ao lado.
        Onde::Raiz => match resposta {
            Json::Objeto(pares)
                if ["rowid", "linha", "versao"]
                    .iter()
                    .all(|c| pares.iter().any(|(k, _)| k == c)) =>
            {
                Json::Objeto(
                    pares
                        .into_iter()
                        .map(|(k, v)| {
                            if k == "linha" {
                                (k, peneirar_linha(v, negadas))
                            } else {
                                (k, v)
                            }
                        })
                        .collect(),
                )
            }
            outra => peneirar_linha(outra, negadas),
        },
        Onde::Lista => match resposta {
            Json::Objeto(pares) => Json::Objeto(
                pares
                    .into_iter()
                    .map(|(k, v)| {
                        if k != "linhas" {
                            return (k, v);
                        }
                        match v {
                            Json::Lista(itens) => (
                                k,
                                Json::Lista(
                                    itens
                                        .into_iter()
                                        .map(|l| peneirar_linha(l, negadas))
                                        .collect(),
                                ),
                            ),
                            outro => (k, outro),
                        }
                    })
                    .collect(),
            ),
            outra => outra,
        },
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// **Os dois sentidos do laco.** Operacao do catalogo sem classe passaria
    /// a ser recusada calada para quem tem regra de coluna; classe sem
    /// operacao e a chave morta -- parece cobertura e nao cobre nada.
    #[test]
    fn a_lista_e_o_catalogo_sao_a_mesma_lista() {
        let mut do_catalogo: Vec<&str> = Vec::new();
        for o in crate::catalogo::OPERACOES {
            do_catalogo.extend(o.nomes());
        }
        let faltando: Vec<&&str> = do_catalogo
            .iter()
            .filter(|n| !CLASSES.iter().any(|(c, _)| c == *n))
            .collect();
        assert!(
            faltando.is_empty(),
            "operacoes sem classe de direito por coluna: {faltando:?}"
        );
        let sobrando: Vec<&str> = CLASSES
            .iter()
            .map(|(n, _)| *n)
            .filter(|n| !do_catalogo.contains(n))
            .collect();
        assert!(
            sobrando.is_empty(),
            "classes para operacoes que nao existem: {sobrando:?}"
        );
    }

    /// **Os dois sentidos do laco da SAIDA** (pedido 245, O5). Operacao
    /// recusada sem saida propria cai no conselho padrao e ninguem percebe --
    /// era exatamente esse o defeito, com uma frase so servindo a todas.
    /// Saida para operacao que NAO recusa e a chave morta: o tradutor a le,
    /// e nada na tela muda.
    #[test]
    fn a_lista_das_saidas_e_a_das_recusadas() {
        let recusadas: Vec<&str> = CLASSES
            .iter()
            .filter(|(_, c)| *c == PorColuna::Recusa)
            .map(|(n, _)| *n)
            .collect();
        let sem_saida: Vec<&&str> = recusadas
            .iter()
            .filter(|n| !SAIDAS.iter().any(|(s, _)| s == *n))
            .collect();
        assert!(
            sem_saida.is_empty(),
            "operacoes recusadas sem saida propria: {sem_saida:?}"
        );
        let sobrando: Vec<&str> = SAIDAS
            .iter()
            .map(|(n, _)| *n)
            .filter(|n| !recusadas.contains(n))
            .collect();
        assert!(
            sobrando.is_empty(),
            "saidas para operacoes que nao recusam: {sobrando:?}"
        );
        for (i, (n, _)) in SAIDAS.iter().enumerate() {
            assert!(
                !SAIDAS[..i].iter().any(|(o, _)| o == n),
                "{n} tem duas saidas"
            );
        }
    }

    /// **O defeito medido.** A frase mandava «peca as colunas por ler, varrer,
    /// buscar ou SELECT» tambem para quem pediu um AGREGADO e para quem pediu
    /// um BACKUP.
    ///
    /// Reponha o defeito fazendo `saida()` devolver sempre
    /// `Saida::PelaLinha`: as duas primeiras asserções caem.
    #[test]
    fn o_agrupar_nao_manda_mais_pedir_as_colunas() {
        assert_eq!(saida("agrupar"), Saida::Resumo);
        assert!(
            !saida("agrupar").texto().contains("varrer"),
            "{}",
            saida("agrupar").texto()
        );
        assert_eq!(saida("backup"), Saida::TabelaInteira);
        assert!(!saida("backup").texto().contains("varrer"));
        // O CONTROLE: quem devolve linha continua recebendo o conselho que
        // sempre serviu -- tirar a frase de todos seria trocar um defeito por
        // outro.
        assert_eq!(saida("juntar"), Saida::PelaLinha);
        assert!(saida("juntar").texto().contains("varrer"));
        // Operacao que ninguem classificou cai no conselho conservador, nunca
        // no que promete um caminho que talvez nao exista.
        assert_eq!(saida("operacao_que_nao_existe"), Saida::TabelaInteira);
    }

    /// Nome repetido na tabela seria duas verdades para a mesma operacao, e a
    /// que ganha e a primeira -- que nao e necessariamente a mais apertada.
    #[test]
    fn nenhuma_operacao_aparece_duas_vezes() {
        for (i, (n, _)) in CLASSES.iter().enumerate() {
            assert!(
                !CLASSES[..i].iter().any(|(o, _)| o == n),
                "{n} esta classificada duas vezes"
            );
        }
    }

    /// As tres que escondem a tabela do portao geral continuam nomeando-a
    /// aqui -- e as tres copias nomeiam o destino.
    #[test]
    fn as_tabelas_escondidas_aparecem() {
        let j =
            Json::analisar(r#"{"database":"b","a":{"tabela":"clientes"},"b":{"tabela":"folha"}}"#)
                .unwrap();
        assert_eq!(tabelas_do_pedido("juntar", &j), vec!["clientes", "folha"]);

        let u = Json::analisar(r#"{"tabelas":["clientes",{"tabela":"folha"}]}"#).unwrap();
        assert_eq!(tabelas_do_pedido("unir", &u), vec!["clientes", "folha"]);

        let p = Json::analisar(r#"{"tabela":"vendas","juntar":[{"tabela":"folha"}]}"#).unwrap();
        assert_eq!(tabelas_do_pedido("pivotar", &p), vec!["vendas", "folha"]);

        let c = Json::analisar(r#"{"tabela":"folha","destino":"copia"}"#).unwrap();
        assert_eq!(
            tabelas_do_pedido("duplicar_tabela", &c),
            vec!["folha", "copia"]
        );
    }

    /// **O braco-PEDIDO do `unir` tambem aparece, e a varredura desce nele.**
    ///
    /// Desde o pedido 393 o braco pode ser um `varrer`, um `buscar` ou um
    /// `consultar` -- e o `consultar` esconde a tabela em mais um nivel. Uma
    /// lista que parasse no campo `tabelas` responderia VAZIO sobre um pedido
    /// que nomeia duas tabelas, e lista vazia aqui quer dizer «nao da para
    /// saber qual», que e outra coisa.
    ///
    /// **Prova real, com o defeito reposto:** tire o laco do `"partes"` de
    /// `tabelas_do_pedido` e este teste devolve `[]`.
    #[test]
    fn o_braco_pedido_da_uniao_aparece() {
        let u = Json::analisar(
            r#"{"partes":[{"op":"buscar","tabela":"clientes","indice":"porUf"},
                          {"op":"consultar","de":{"op":"varrer","tabela":"folha"}}]}"#,
        )
        .unwrap();
        assert_eq!(tabelas_do_pedido("unir", &u), vec!["clientes", "folha"]);
    }

    /// **Os cinco caminhos do `consultar` aparecem, e a varredura DESCE.**
    ///
    /// `existe[].de` e o campo que entrou em 09/09/2026: quando o portao
    /// passa a olhar um campo novo, procure quem nao tem esse campo -- e este
    /// teste e o que cai se alguem acrescentar um sexto caminho e esquecer
    /// desta lista.
    #[test]
    fn os_cinco_caminhos_do_consultar_aparecem() {
        let c = Json::analisar(
            r#"{"de":{"op":"consultar","de":{"op":"varrer","tabela":"pedidos"}},
                "juntar":[{"de":{"op":"varrer","tabela":"clientes"}}],
                "escalar":[{"de":{"op":"agrupar","tabela":"limites"}}],
                "em":[{"de":{"op":"varrer","tabela":"cidades"}}],
                "existe":[{"de":{"op":"varrer","tabela":"folha"}}]}"#,
        )
        .unwrap();
        assert_eq!(
            tabelas_do_pedido("consultar", &c),
            vec!["pedidos", "clientes", "limites", "cidades", "folha"]
        );
    }

    #[test]
    fn a_peneira_tira_a_coluna_das_duas_formas_do_ler() {
        let negadas = vec!["salario".to_string()];
        let crua = Json::analisar(r#"{"id":1,"nome":"ana","salario":"10.00"}"#).unwrap();
        let r = peneirar(crua, Onde::Raiz, &negadas);
        assert!(r.campo("salario").is_none(), "{}", r.escrever());
        assert_eq!(r.texto_ou("nome", ""), "ana");

        let com_versao =
            Json::analisar(r#"{"rowid":1,"linha":{"id":1,"salario":"10.00"},"versao":3}"#).unwrap();
        let r = peneirar(com_versao, Onde::Raiz, &negadas);
        assert!(r.campo("linha").unwrap().campo("salario").is_none());
        assert_eq!(r.inteiro_ou("versao", 0), 3);
    }

    /// A caixa do `config.json` nao decide seguranca: `Salario` e `salario`
    /// sao a mesma coluna, e uma regra que falha por uma maiuscula e pior que
    /// uma regra que nao existe.
    #[test]
    fn a_peneira_nao_se_perde_na_caixa() {
        let negadas = vec!["SALARIO".to_string()];
        let r = peneirar(
            Json::analisar(r#"{"linhas":[{"rowid":1,"salario":"10.00"}]}"#).unwrap(),
            Onde::Lista,
            &negadas,
        );
        let l = &r.campo("linhas").and_then(Json::lista).unwrap()[0];
        assert!(l.campo("salario").is_none(), "{}", r.escrever());
        assert_eq!(l.inteiro_ou("rowid", 0), 1);
    }
}
