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
            // O catalogo entra aqui, junto com o `quem_sou`, e nao entre as
            // que pedem `ler`: ele descreve o PROTOCOLO, que e documentacao
            // publica, e nao dado. Quem exige poder para ver o catalogo tira a
            // ajuda de quem so insere -- e nao esconde nada, porque a lista de
            // operacoes ja esta no MANUAL. O que ele mostra e filtrado pelo
            // poder de quem perguntou, entao a resposta nunca promete mais do
            // que aquela sessao consegue chamar.
            "ping" | "login" | "desafio" | "quem_sou" | "sair" | "catalogo" => return None,
            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "coletar_rowids" | "buscar" => {
                Atividade::Ler
            }
            // Procurar uma palavra le a tabela por outro caminho, e devolve as
            // MESMAS linhas que o `varrer` devolveria -- entao pede o mesmo
            // poder. Pedir mais aqui seria esconder pela indexacao um dado que
            // a varredura ja entrega a quem pode ler.
            "procurar_texto" => Atividade::Ler,
            // O catalogo e leitura: quem pode ler a tabela pode saber que ela
            // existe e que colunas tem.
            "sistabelas" | "systables" | "siscolunas" | "syscolumns" => Atividade::Ler,
            // O mapa de dado pessoal e catalogo, e nao dado: mostra QUE a
            // coluna guarda CPF, nunca o CPF. Pede `ler` pelo mesmo motivo
            // que `siscolunas` -- e, como varre a base inteira sem campo
            // "tabela", a propria operacao filtra tabela a tabela por dentro.
            "dados_pessoais" | "lgpd" => Atividade::Ler,
            // O pivot resume o que a varredura leria: quem pode ler a tabela
            // pode ver o total dela. O `agrupar` e o mesmo argumento: um total
            // por cidade nao diz nada que a varredura ja nao entregasse linha
            // a linha -- e pedir mais aqui esconderia por agregacao um dado
            // que quem pode ler ja ve inteiro.
            "pivotar" | "pivot" | "agrupar" | "group_by" => Atividade::Ler,
            // Junção e união leem duas ou mais tabelas da MESMA base, e o
            // poder de ler vale por base -- entao ler e o suficiente, e a
            // operacao confere de novo antes de abrir a segunda tabela.
            "juntar" | "join" | "unir" | "union" => Atividade::Ler,
            // Comparar duas tabelas e LER as duas: a resposta traz linhas
            // delas. Como o `juntar` e o `unir`, a operacao confere de novo por
            // dentro, porque as tabelas dela moram em campos que este portao
            // nao olha.
            "diferencas" | "diff" => Atividade::Ler,
            "sequencias" | "sequences" => Atividade::Ler,
            // A op `sql` so produz `varrer` e `buscar` hoje, e as duas pedem
            // `ler`. Este portao e o de FORA e nao dispensa o de dentro: o
            // pedido traduzido volta pelo mesmo `portoes_do_pedido`, com o
            // nome da tabela no campo que ele ja sabe olhar. Entao ele so pode
            // apertar, nunca afrouxar -- e apertar e o lado certo de errar.
            //
            // O preco esta escrito em docs/SQL.md: quem so tem direito por
            // TABELA, e nenhum na base, para aqui. Nao da para consertar sem o
            // portao ler o texto do SQL, e portao que interpreta linguagem e
            // portao que erra.
            "sql" => Atividade::Ler,
            // O `consultar` compoe, e nao le: cada sub-pedido dele volta pelo
            // `executar_derivado` com a tabela DELE no campo que o portao ja
            // olha. Este portao e o de FORA e nao dispensa o de dentro -- ele
            // so pode apertar, e `ler` e o piso certo: quem nao pode ler nada
            // nao tem o que compor.
            "consultar" => Atividade::Ler,
            // Listar as visoes e LER o catalogo do banco: o texto de um
            // `SELECT` diz que tabelas existem, e nao o que ha nelas. Ja
            // CRIAR uma exige `criar`, o mesmo poder de criar tabela, e
            // EXCLUIR exige `excluir`: quem apaga uma visao apaga a consulta
            // de todo mundo que a usa.
            "visoes" => Atividade::Ler,
            "criar_visao" => Atividade::Criar,
            "excluir_visao" => Atividade::Excluir,
            // A soma de verificacao le a tabela inteira e devolve um numero:
            // quem pode ler a tabela pode saber se ela mudou.
            "checksum" | "soma_de_verificacao" => Atividade::Ler,
            // Exportar e ler a tabela inteira e levar embora. Nao e mais poder
            // do que `varrer` ja da -- e menos, porque nao altera nada.
            "exportar" | "export" => Atividade::Ler,
            // Mexer no contador pode fazer a proxima insercao repetir numero.
            "ajustar_sequencia" => Atividade::Administrar,
            // Consultar em memoria e ler: o dado e o mesmo, o caminho e outro.
            // Carregar tambem, porque carregar e varrer a tabela inteira.
            "memoria_carregar" | "memoria" | "SelectMemory" | "selectmemory"
            | "selecionar_memoria" => Atividade::Ler,
            // O painel conta so o que quem olha poderia abrir, entao pede
            // leitura -- e nao administrar. Um operador tem direito de ver o
            // tamanho do que ele mesmo opera.
            "painel" => Atividade::Ler,
            // A saude do disco anda com o painel: o bloco reduzido (estado,
            // tempos, tipo do ultimo erro) e para quem le; o texto do erro e
            // o alvo dele so saem para quem administra, dentro da propria op.
            "saude_disco" => Atividade::Ler,
            // Ja o monitor da MAQUINA pede administrar. Nome de placa de rede,
            // nome de disco e ponto de montagem descrevem a infraestrutura, e
            // nao o dado -- quem so le uma tabela nao ganha nada com isso e o
            // atacante ganha o mapa.
            "sistema" => Atividade::Administrar,
            // DbLink inteiro exige administrar, inclusive o que so LE do
            // outro banco. Uma ligacao guarda UMA credencial, e quem a usa
            // fala com o outro servidor como aquele usuario -- as permissoes
            // por base do PhxSql nao atravessam. Deixar um leitor navegar por
            // ela seria emprestar o poder de quem a criou.
            op if op.starts_with("dblink") => Atividade::Administrar,
            // Job inteiro exige administrar, inclusive so LER a lista, e pelo
            // mesmo motivo do DbLink: um job carrega o login sob o qual roda e
            // o pedido inteiro que executa. Ver a lista e ver que operacao roda
            // sobre que tabela de quem; poder salvar um e poder mandar o
            // servidor executar qualquer coisa com o poder de outro usuario.
            // Isso nao pode ser direito de leitor.
            // O apelido `job_listar` e o `job_ligar` entram AQUI junto com o
            // resto: operacao que o portao nao mapeia e operacao que anonimo
            // chama -- e o furo mora sempre no nome que alguem esqueceu.
            "jobs" | "job_listar" | "job_salvar" | "job_excluir" | "job_rodar" | "job_ligar" => {
                Atividade::Administrar
            }
            // Backup e restauracao sao da familia do `acessos` e do `config`:
            // administrar o SERVIDOR. Estao aqui declaradas, e nao so caindo
            // no `_`, pelo mesmo motivo do `config_gravar` -- a operacao que
            // grava um database inteiro e a ultima que deveria depender do
            // padrao para negar.
            //
            // E o portao geral NAO basta para o `restaurar_backup`: ele confere
            // o campo "database", que ali e o DESTINO; o database que vem
            // DENTRO do backup nao tem campo no pedido, e e ele que carrega o
            // dado. Quem confere esse e o `poder_no_backup`, dentro da
            // operacao -- a licao do `juntar` e do `unir`.
            "backup" | "conferir_backup" | "backups" | "restaurar_backup" => Atividade::Administrar,
            // Parar e subir a porta de dados e o poder mais bruto que ha aqui:
            // um clique tira o servico do ar para todo mundo. Administrar, e
            // nada menos.
            "servico" | "servico_parar" | "servico_subir" => Atividade::Administrar,
            // Os textos da tela moram numa tabela comum, entao ver o estado e
            // tirar o backup sao LEITURA -- exatamente como `exportar`, que ja
            // le a tabela inteira e leva embora.
            //
            // Este portao confere a base VAZIA, porque nenhuma das cinco tem
            // campo "tabela" no pedido. Ele so aperta; quem confere a tabela
            // de verdade e o `poder_nos_idiomas`, dentro de cada operacao.
            "idiomas" | "idiomas_exportar" => Atividade::Ler,
            // As tres que ESCREVEM na tabela de textos. Semear nao desfaz
            // traducao de ninguem, mas `idiomas_padrao` e `idiomas_importar`
            // gravam por cima -- e quem pode reescrever o texto de toda tela
            // do sistema esta administrando, nao alterando um registro.
            "idiomas_carga" | "idiomas_padrao" | "idiomas_importar" => Atividade::Administrar,
            "inserir" => Atividade::Inserir,
            // Reservar a tabela para carga exige o poder de INSERIR nela, e
            // nao mais: quem pode gravar mil linhas pode pedir a tabela para
            // gravar mil linhas. Ja `cargas` -- a lista de quem reservou o que
            // -- mostra o movimento dos outros, e por isso pede administrar,
            // pela mesma razao que `sessoes` pede.
            "bulkinsert" => Atividade::Inserir,
            "cargas" => Atividade::Administrar,
            // O CONTROLE de transacao nao pede poder nenhum, e isso e
            // deliberado: abrir uma transacao nao da acesso a nada. Cada
            // escrita empilhada volta pelo portao de sempre com a atividade
            // dela (`inserir`, `alterar`, `excluir`) e com a tabela do pedido,
            // entao abrir nunca da poder que a pessoa ja nao tinha -- o mesmo
            // argumento do `CALL`.
            //
            // `transacao` (o estado da PROPRIA conexao) entra aqui pelo mesmo
            // motivo do `quem_sou`: e a ficha de quem esta perguntando.
            "begin"
            | "start_transaction"
            | "begin_transaction"
            | "commit"
            | "rollback"
            | "savepoint"
            | "rollback_para"
            | "rollback_to_savepoint"
            | "release_savepoint"
            | "transacao" => return None,
            // Ja `transacoes` -- a lista de quem segura o que -- mostra o
            // movimento dos outros, e por isso pede administrar, pela mesma
            // razao que `cargas` e `sessoes` pedem.
            "transacoes" => Atividade::Administrar,
            // Carga em lote e insercao, e nao mais que isso: quem pode gravar
            // uma linha pode gravar mil. O que muda e o custo, e para isso ha
            // o teto de linhas por carga.
            "inserir_lote" | "importar" | "carga" => Atividade::Inserir,
            // Conferir LE a carga que o proprio usuario colou e le o esquema
            // da tabela: nao grava nada, e por isso pede so `ler`. Barrar
            // aqui obrigaria a tentar gravar para descobrir se a carga serve.
            "importar_conferir" => Atividade::Ler,
            "atualizar" => Atividade::Alterar,
            "excluir" => Atividade::Excluir,
            // Restaurar e desfazer uma exclusao: exige o mesmo poder de
            // excluir, e nao mais. Quem pode tirar da lista pode devolver.
            "restaurar" => Atividade::Excluir,
            // O `.trash` e o `.reason` sao dos administradores, e a razao esta
            // no conteudo dos dois. O `.trash` guarda o dado que alguem mandou
            // apagar -- quem so tem `ler` perdeu o direito de ver aquela linha
            // no instante em que ela foi excluida, e a lixeira devolveria o
            // direito por outra porta. O `.reason` costuma ser ainda mais
            // revelador que o registro: "fraude", "pedido de remocao do
            // titular", "duplicidade com o contrato X".
            "lixeira" | "trash" | "motivos" | "reasons" => Atividade::Administrar,
            // A trilha de LGPD e dos administradores pelo mesmo argumento,
            // levado ao extremo: ela guarda o VALOR de antes e o de depois das
            // colunas marcadas como dado pessoal. Quem a le nao le "houve uma
            // alteracao"; le o CPF velho e o CPF novo, o endereco velho e o
            // novo, de todas as linhas de uma vez. E o arquivo mais revelador
            // da tabela, e o unico que concentra o que a lei manda proteger --
            // dar acesso a ele por `ler` seria abrir por uma porta lateral
            // tudo o que a permissao por tabela fecha pela porta da frente.
            "trilha" | "trilha_lgpd" => Atividade::Administrar,
            // Classificar coluna e mexer no ESQUEMA, e desmarcar uma coluna
            // apaga a trilha dela daqui para a frente -- que e a maneira mais
            // silenciosa que existe de desligar a auditoria de um campo.
            // Administrar, como toda mudanca de estrutura.
            "marcar_lgpd" | "marcar_dado_pessoal" => Atividade::Administrar,
            // Esvaziar a lixeira e a unica operacao do motor que apaga dado
            // sem rede nenhuma embaixo.
            "esvaziar_lixeira" => Atividade::Administrar,
            // Expurgar a trilha apaga a PROVA de quem mexeu e quem viu o dado
            // pessoal -- e quem a le ja precisa administrar (`trilha`, acima).
            // Pedir menos para apagar do que para ler seria a porta dos fundos.
            "expurgar_trilha" => Atividade::Administrar,
            "criar_database" | "criar_schema" | "criar_tabela" | "duplicar_tabela"
            | "copiar_tabela" => Atividade::Criar,
            // Declarar e desdeclarar chave estrangeira e desenhar o MODELO, e
            // nao mexer em dado: a chave e catalogo e o motor nao a impoe (ha
            // teste que trava isso). Pede o mesmo poder de quem cria tabela --
            // que sempre pode declara-la no proprio criar_tabela -- e o portao
            // por tabela vale, porque as duas operacoes tem o campo "tabela".
            "declarar_fk" | "excluir_fk" => Atividade::Criar,
            // Acrescentar coluna reescreve o `.reg` INTEIRO -- e a maior
            // escrita de estrutura do motor, e a que nao tem desfazer barato.
            // Nao basta poder criar tabela: isto exige administrar, como o
            // `marcar_lgpd` e o `excluir_tabela` ao lado.
            "acrescentar_coluna" => Atividade::Administrar,
            // Levar a tabela ao PSCH v10 e o `acrescentar_coluna` duas vezes,
            // entao nao pode pedir menos que ele. E o campo que o portao le e
            // o `tabela` de sempre -- com uma excecao que a propria operacao
            // paga: sem `tabela`, ela VARRE a base para dizer quais faltam, e
            // ali o portao geral so conferiu a BASE. A varredura confere
            // tabela a tabela por dentro, como o `dados_pessoais` ao lado.
            "migrar_esquema" => Atividade::Administrar,
            // Apagar uma tabela apaga os cinco arquivos de uma vez, e nao ha
            // desfazer. Nao basta poder excluir LINHA para poder excluir a
            // TABELA: isto exige administrar.
            "excluir_tabela" => Atividade::Administrar,
            "reindexar" => Atividade::Reindexar,
            "diario" => Atividade::Diario,
            // Aplicar GRAVA na tabela, e grava por fora das conferencias
            // normais: rowid escolhido do evento, payload cru vindo de fora.
            // Nao e insercao comum, e por isso pede o poder de administrar e
            // nao o de inserir.
            "aplicar" => Atividade::Administrar,
            "verificar" => Atividade::Verificar,
            // As estatisticas resumem o log de acessos, que ja exige
            // administrar: quem ve quanto cada usuario pediu ve o movimento
            // dos outros.
            "estatisticas" | "estatisticas_uso" => Atividade::Administrar,
            // Ver quem esta conectado e derrubar conexao sao poder de
            // administrador: a lista mostra o login e o IP dos outros, e
            // derrubar interrompe o trabalho alheio.
            "sessoes" | "processlist" | "encerrar_sessao" | "kill" => Atividade::Administrar,

            // A exportacao e a whitelist sao a mesma familia do `bloqueios`:
            // quem ve e solta IP tambem exporta e protege. E a gestao das
            // mensagens e configuracao do servidor -- soltar um texto errado
            // na resposta de todo mundo nao e direito de leitor.
            "bloqueios_exportar" | "whitelist_salvar" | "mensagens" | "mensagens_semear" => {
                Atividade::Administrar
            }
            // A telemetria mostra o login, o IP e a TABELA de toda atividade
            // viva, e o encerrar interrompe o trabalho alheio: e o mesmo poder
            // do `sessoes` e do `kill`, pelas mesmas duas razoes.
            //
            // Este portao ve a base VAZIA, porque nenhuma das quatro tem campo
            // "database" -- entao ele nao basta, e as quatro conferem por
            // dentro se quem pediu e administrador DESTE SERVIDOR. E a licao
            // do juntar/unir: quando o portao geral nao consegue enxergar o
            // campo, a operacao pergunta por conta.
            "telemetria" | "telemetria_ligar" | "telemetria_desligar" | "telemetria_encerrar" => {
                Atividade::Administrar
            }
            // `config_gravar` esta aqui declarado, e nao so caindo no `_`:
            // a operacao que reescreve o config.json e a ultima que deveria
            // depender do padrao para negar. A op ainda confere por dentro.
            // `diretivas` e `diretiva_gravar` entram aqui pelo mesmo motivo do
            // `config_gravar`: as duas falam da configuracao do SERVIDOR, nao
            // de dado, e nenhuma delas tem campo "tabela" para o portao geral
            // olhar. As duas conferem por dentro tambem, pelo mesmo portao
            // unico (`exigir_administrar_config`).
            "acessos" | "ips" | "config" | "config_gravar" | "diretivas" | "diretiva_gravar"
            | "usuarios" | "bloqueios" | "desbloquear" => Atividade::Administrar,
            // As tres que ESCREVEM o cadastro. Declaradas, e nao caindo no
            // `_`, pelo mesmo motivo do `config_gravar` ao lado: a operacao
            // que cria quem pode entrar e a ultima que deveria depender do
            // padrao para negar.
            //
            // As tres conferem de novo por dentro, e a repeticao nao e
            // duplicacao: nenhuma delas tem campo "database", entao este
            // portao as ve na base VAZIA e o poder sai da regra `"*"` ou do
            // nivel. E a licao do `juntar` -- quando o portao geral nao
            // enxerga o campo, a operacao pergunta por conta.
            "usuario_criar" | "usuario_alterar" | "usuario_excluir" => Atividade::Administrar,
            // O profiler mostra o TEXTO dos pedidos de todo mundo, com os
            // dados que estao sendo gravados dentro. Quem pode ler uma tabela
            // nao ganha por isso o direito de ver o que os outros escrevem
            // nela -- nem de mandar o servidor escrever um arquivo no disco.
            "profiler" | "profiler_ligar" | "profiler_desligar" | "profiler_limpar" => {
                Atividade::Administrar
            }
            // O fluxo de replicacao e o diario com a linha inteira dentro:
            // permissao propria, para poder dar a uma replica sem dar mais
            // nada -- e para nao sair de graca junto com `ler`.
            "posicao" | "replicar" => Atividade::Replicar,
            // O pulso e conversa entre nos do cluster, autenticada como a
            // replicacao -- nunca anonima: quem pode pedir o fluxo pode dizer
            // que esta vivo. Ja o estado e leitura: e o endereco unico do
            // cluster, e qualquer cliente precisa dele para achar o master.
            "cluster_pulso" => Atividade::Replicar,
            "cluster_estado" => Atividade::Ler,
            // Mexer na LISTA de nos muda o denominador da maioria -- quem
            // acrescenta um no muda quantos votos fazem um master. Isso e
            // decisao de quem manda no servidor, e nao da credencial de
            // replicacao que ja basta para pulsar: com `Replicar` aqui,
            // qualquer replica poderia inflar o cluster com nos fantasmas e
            // travar toda promocao. As duas ops conferem por dentro tambem.
            "cluster_no_acrescentar" | "cluster_no_remover" => Atividade::Administrar,
            // Promover um spare vira o papel do servidor inteiro; o estado do
            // laco expoe origem, endereco e erro de conexao. Os dois sao
            // decisao e mapa de administrador, nao de replica.
            // `replicacao_ligar` religa um laco que parou por credencial
            // recusada: e o mesmo mapa de administrador.
            // `replicacao_pular` DESCARTA um evento do outro lado: e a
            // operacao mais perigosa das cinco, e vai no mesmo poder porque
            // acima de administrar nao ha. O portao dela le o campo
            // `"tabela"` do pedido, que ela obriga -- por isso ela nao entra
            // na lista das que escondem tabela do portao.
            "spare_promover" | "replicacao_estado" | "replicacao_testar" | "replicacao_ligar"
            | "replicacao_pular" => Atividade::Administrar,
            // ------------------------------------------------------------
            // As 13 que o conferidor `toda_operacao_do_catalogo_declara_o_
            // poder_que_pede` achou caindo no `_`.
            //
            // **Nenhum poder muda aqui**: todas ja exigiam administrar, por
            // omissao, e e esse mesmo valor que passa a estar escrito. Mudar
            // qualquer uma agora tiraria ou daria direito sem ninguem ter
            // pedido -- e a lei da casa e clara sobre isso. O que muda e so o
            // silencio: quem quiser rebaixar uma delas passa a ter de dizer.
            // ------------------------------------------------------------
            //
            // DDL e reparo: mexem na tabela como objeto, e nao no dado dela.
            "renomear_tabela" | "reparar" => Atividade::Administrar,
            // Soltar a tabela em memoria de OUTRA sessao e decisao de quem
            // manda no servidor, e nao de quem consulta -- por isso ela nao
            // acompanha o `memoria`, que e leitura.
            "memoria_liberar" => Atividade::Administrar,
            // O DBLINK inteiro: a ligacao carrega CREDENCIAL de outro
            // servidor, e o poder aqui e sobre a credencial, nao sobre a
            // tabela. Quem pode ler uma tabela daqui nao ganha por isso o
            // direito de sair pela rede com a senha que o administrador
            // guardou -- inclusive as de LER, que e o ponto: o dado do outro
            // lado nao tem portao nosso.
            "dblink_salvar" | "dblink_excluir" | "dblink_testar" | "dblink_bancos"
            | "dblink_tabelas" | "dblink_estrutura" | "dblink_ler" | "dblink_consultar"
            | "dblink_ligar" | "dblink_sincronizar" => Atividade::Administrar,
            // Operacao desconhecida exige o maior poder: nega por omissao.
            _ => Atividade::Administrar,
        })
    }
}

/// Nivel do usuario: um nome no lugar de dez booleanos.
///
/// Escrever dez permissoes por base, para cada usuario, e onde alguem erra --
/// esquece uma, deixa `administrar` ligado sem querer, copia a linha errada.
/// O nivel resolve o caso comum com uma palavra, e as permissoes por base
/// continuam la para o caso que o nivel nao cobre.
///
/// A ordem importa: cada nivel contem o anterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Nivel {
    /// Nada. E o padrao quando o `config.json` nao diz nivel nenhum.
    ///
    /// Existe porque a regra do projeto e negar por omissao, e sem este nivel
    /// o padrao viraria "le tudo" -- todo config que ja existe passaria a dar
    /// leitura em base que antes negava. Um teste antigo pegou exatamente
    /// isso, e este nivel e a correcao.
    #[default]
    Nenhum,
    /// So le.
    Leitor,
    /// Le e escreve, mas nao cria base nem mexe em indice.
    Operador,
    /// Tudo sobre os dados: cria, reindexa, replica.
    Dono,
    /// Tudo, mais o servidor: acessos, bloqueios, usuarios, backup.
    Admin,
}

impl Nivel {
    pub fn de_texto(s: &str) -> Result<Nivel> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "nenhum" | "nada" => Nivel::Nenhum,
            "leitor" | "consulta" | "leitura" => Nivel::Leitor,
            "operador" | "operacao" | "escrita" => Nivel::Operador,
            "dono" | "owner" | "proprietario" => Nivel::Dono,
            "admin" | "administrador" | "dba" => Nivel::Admin,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "nivel desconhecido: {outro:?} (use leitor, operador, dono ou admin)"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Nivel::Nenhum => "nenhum",
            Nivel::Leitor => "leitor",
            Nivel::Operador => "operador",
            Nivel::Dono => "dono",
            Nivel::Admin => "admin",
        }
    }

    /// O que este nivel pode, numa base.
    pub fn permissoes(self) -> Permissoes {
        if self == Nivel::Nenhum {
            return Permissoes::default();
        }
        let mut p = Permissoes {
            ler: true,
            diario: true,
            verificar: true,
            ..Permissoes::default()
        };
        if self >= Nivel::Operador {
            p.inserir = true;
            p.alterar = true;
            p.excluir = true;
        }
        if self >= Nivel::Dono {
            p.criar = true;
            p.reindexar = true;
            p.replicar = true;
        }
        if self >= Nivel::Admin {
            p.administrar = true;
        }
        p
    }
}

impl PartialOrd for Nivel {
    fn partial_cmp(&self, outro: &Nivel) -> Option<std::cmp::Ordering> {
        Some(self.cmp(outro))
    }
}

impl Ord for Nivel {
    fn cmp(&self, outro: &Nivel) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*outro as u8))
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

/// O direito sobre uma COLUNA de uma tabela.
///
/// So duas atividades cabem aqui, e a escolha e do dado, nao de gosto:
/// `ler` decide se a coluna sai na resposta e `alterar` decide se ela pode
/// mudar de valor. As outras oito da base nao tem significado por coluna --
/// nao se `reindexa` um campo nem se `administra` metade de uma linha --, e
/// por isso um direito desconhecido dentro de `colunas` **recusa a carga do
/// cadastro** em vez de ser ignorado: campo que ninguem le mente, e mente
/// pior quando o assunto e quem alcanca o dado.
///
/// Nega por omissao, como a base e a tabela: `{"salario": {}}` tira as duas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DireitoDeColuna {
    pub ler: bool,
    pub alterar: bool,
}

impl DireitoDeColuna {
    pub fn pode(self, atividade: Atividade) -> bool {
        match atividade {
            Atividade::Ler => self.ler,
            Atividade::Alterar => self.alterar,
            // As outras oito nao existem por coluna, e responder `false` aqui
            // faria `colunas_negadas` devolver a tabela inteira para uma
            // pergunta que nao se faz. Quem pergunta por elas nao tem
            // restricao de coluna nenhuma -- que e a verdade.
            _ => true,
        }
    }

    fn de_json(j: &Json, onde: &str) -> Result<DireitoDeColuna> {
        if let Json::Objeto(pares) = j {
            for (chave, _) in pares {
                if chave != "ler" && chave != "alterar" {
                    return Err(PhxError::Esquema(format!(
                        "{onde}: direito {chave:?} nao existe por coluna; \
                         por coluna so ha \"ler\" e \"alterar\""
                    )));
                }
            }
        } else {
            return Err(PhxError::Esquema(format!(
                "{onde}: o direito de uma coluna e um objeto \
                 {{\"ler\": false, \"alterar\": false}}"
            )));
        }
        Ok(DireitoDeColuna {
            ler: j.booleano_ou("ler", false),
            alterar: j.booleano_ou("alterar", false),
        })
    }

    fn para_json(self) -> Json {
        Json::objeto(vec![
            ("ler", Json::Bool(self.ler)),
            ("alterar", Json::Bool(self.alterar)),
        ])
    }
}

/// As regras de coluna de uma TABELA: coluna -> direito.
pub type RegrasDeColuna = Vec<(String, DireitoDeColuna)>;

/// As regras de coluna de uma BASE: tabela (ou `"*"`) -> [`RegrasDeColuna`].
pub type ColunasDaBase = Vec<(String, RegrasDeColuna)>;

/// O direito por coluna inteiro: base (ou `"*"`) -> [`ColunasDaBase`].
///
/// Tres niveis porque a precedencia tem tres niveis, e ela e a MESMA de
/// [`Usuario::permissoes_em`] -- achatar em uma chave `"base.tabela.coluna"`
/// custaria montar uma `String` por pergunta e perderia o `"*"` de cada
/// nivel.
pub type DireitoPorColuna = Vec<(String, ColunasDaBase)>;

#[derive(Clone)]
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
    /// Nivel: o poder que este usuario tem nas bases onde nao ha regra
    /// explicita. `bases` continua mandando, quando existe.
    pub nivel: Nivel,
    /// Chave publica Ed25519, se este usuario tambem prova posse de chave.
    ///
    /// A senha prova que ele SABE alguma coisa; a chave prova que ele TEM
    /// alguma coisa. Quem copiar o config.json leva so a publica, que nao
    /// assina nada -- e a diferenca em relacao ao hash da senha, que e
    /// exatamente o que o desafio-resposta usa para autenticar.
    pub chave_publica: Option<[u8; phxsql_core::ed25519::CHAVE_LEN]>,
    /// Poder por base. A chave `"*"` vale para as bases nao listadas.
    pub bases: Vec<(String, Permissoes)>,
    /// Poder por TABELA, dentro de cada base.
    ///
    /// Chave de fora: a base (ou `"*"`). Chave de dentro: a tabela (ou `"*"`).
    /// Vem de `"tabelas"` dentro do objeto da base, no `config.json`:
    ///
    /// ```json
    /// "bases": {
    ///   "Z": {
    ///     "ler": true, "inserir": true,
    ///     "tabelas": {
    ///       "folha":    { },
    ///       "clientes": { "ler": true, "inserir": true, "alterar": true }
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// # Por que e um campo separado, e nao um campo dentro de `bases`
    ///
    /// Se a regra da tabela morasse dentro do objeto da base, listar uma base
    /// so para escrever uma regra de tabela nela passaria a NEGAR tudo o mais
    /// naquela base -- porque a base listada ganha da regra `"*"`, e o objeto
    /// listado so por causa das tabelas teria as dez permissoes em `false`.
    /// Separado, a precedencia de base fica exatamente como era.
    pub tabelas: Vec<(String, Vec<(String, Permissoes)>)>,
    /// Direito por COLUNA, dentro de cada tabela de cada base.
    ///
    /// Base (ou `"*"`) -> tabela (ou `"*"`) -> coluna -> direito. Vem de
    /// `"colunas"` dentro do objeto da tabela, no `config.json`:
    ///
    /// ```json
    /// "bases": { "Z": { "ler": true, "alterar": true, "tabelas": {
    ///   "folha": { "ler": true, "alterar": true,
    ///              "colunas": { "salario": { "ler": false, "alterar": false } } }
    /// }}}
    /// ```
    ///
    /// **Sem `colunas`, nada muda** -- e a lista fica vazia, que e o que faz
    /// [`Usuario::restringe_colunas`] responder `false` e o servidor inteiro
    /// nao pagar nada por uma funcionalidade que ninguem pediu.
    ///
    /// # Por que aqui e nao dentro de `tabelas`
    ///
    /// Pelo mesmo motivo de `tabelas` nao morar dentro de `bases`: a busca de
    /// permissao PARA e na primeira regra que casa, e uma tabela listada so
    /// para escrever uma regra de coluna nela passaria a substituir a regra da
    /// base. Separado, a precedencia de tabela fica exatamente como era.
    pub colunas: DireitoPorColuna,
}

/// `Debug` a mao: o `senha_hash` NAO e um resumo inofensivo -- e dele que sai
/// a chave do desafio-resposta, entao quem o tem se autentica como o usuario
/// sem nunca saber a senha. E a mesma razao pela qual a ficha do protocolo ja
/// o escondia; faltava a saida do `Debug`.
///
/// `chave_publica` fica visivel: publica e para ser vista, e e ela que permite
/// conferir de fora qual chave o cadastro aceita.
impl std::fmt::Debug for Usuario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Desestruturar SEM `..`: campo novo para de compilar aqui, e quem
        // o acrescentar decide na hora se e segredo. Lista de campos escrita
        // a mao envelhece calada.
        let Usuario {
            id,
            nome,
            login,
            senha_hash: _,
            email,
            telefone,
            supervisor,
            ativo,
            nivel,
            chave_publica,
            bases,
            tabelas,
            colunas,
        } = self;
        f.debug_struct("Usuario")
            .field("id", id)
            .field("nome", nome)
            .field("login", login)
            .field("senha_hash", &"(oculto)")
            .field("email", email)
            .field("telefone", telefone)
            .field("supervisor", supervisor)
            .field("ativo", ativo)
            .field("nivel", nivel)
            .field("chave_publica", chave_publica)
            .field("bases", bases)
            .field("tabelas", tabelas)
            .field("colunas", colunas)
            .finish()
    }
}

impl Usuario {
    /// Permissoes efetivas numa base.
    /// O poder deste usuario nesta base.
    ///
    /// Ordem de precedencia, do mais especifico para o mais geral:
    /// supervisor, a regra da base, a regra `"*"`, e por fim o nivel. O
    /// especifico ganha do geral -- e o que permite dar `admin` a alguem e
    /// ainda assim tirar uma base especifica dele.
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
        self.nivel.permissoes()
    }

    /// E administrador? Vale para operacao de servidor, que nao tem base.
    pub fn e_admin(&self) -> bool {
        self.supervisor || self.nivel >= Nivel::Admin
    }

    /// O poder deste usuario nesta TABELA desta base.
    ///
    /// # Por que existe
    ///
    /// Ate a 0.17.0 a permissao parava na base: quem lia a base lia todas as
    /// tabelas dela, e nao havia como dar `clientes` sem dar `folha`. A folha
    /// de pagamento e a tabela de clientes moram no mesmo banco porque o
    /// negocio e um so, e o direito de ler as duas nao e o mesmo direito.
    ///
    /// # A ordem de precedencia
    ///
    /// A mesma regra que ja valia entre base e `"*"`: **o especifico ganha do
    /// geral, e substitui**. Do mais especifico para o mais geral:
    ///
    /// 1. supervisor -- pode tudo, em toda tabela;
    /// 2. a regra desta tabela nesta base;
    /// 3. a regra `"*"` de tabela nesta base;
    /// 4. a regra desta tabela na base `"*"`;
    /// 5. a regra `"*"` de tabela na base `"*"`;
    /// 6. e so entao a regra da BASE (que por sua vez cai em `"*"` e no nivel).
    ///
    /// Substituir, e nao interceder, e o que permite os dois casos que a
    /// pratica pede: **tirar** uma tabela de quem le a base inteira, e **dar**
    /// uma tabela a quem nao le a base nenhuma.
    ///
    /// Tabela vazia -- operacao que nao fala de tabela, como `bancos` ou
    /// `criar_database` -- cai direto na regra da base.
    pub fn permissoes_em(&self, database: &str, tabela: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if !tabela.is_empty() {
            for base in [database, "*"] {
                if let Some((_, regras)) = self.tabelas.iter().find(|(b, _)| b == base) {
                    for alvo in [tabela, "*"] {
                        if let Some((_, p)) = regras.iter().find(|(t, _)| t == alvo) {
                            return *p;
                        }
                    }
                }
            }
        }
        self.permissoes(database)
    }

    /// Pode fazer a atividade nesta base?
    pub fn pode(&self, database: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes(database).pode(atividade)
    }

    /// Pode fazer a atividade nesta tabela desta base?
    ///
    /// Tabela vazia e o mesmo que perguntar so pela base.
    pub fn pode_em(&self, database: &str, tabela: &str, atividade: Atividade) -> bool {
        self.ativo && self.permissoes_em(database, tabela).pode(atividade)
    }

    /// Este usuario tem ALGUMA regra de coluna, em qualquer base?
    ///
    /// E o interruptor que faz o direito por coluna custar zero para todo
    /// mundo que nao o pediu: e um `bool` lido da ficha da sessao, antes de
    /// qualquer trabalho. Supervisor responde `false` porque ele ja ignora o
    /// portao por tabela -- inventar um segundo caminho de excecao aqui faria
    /// os dois portoes poderem discordar.
    pub fn restringe_colunas(&self) -> bool {
        !self.supervisor && !self.colunas.is_empty()
    }

    /// As regras de coluna que valem nesta tabela desta base.
    ///
    /// A precedencia e a MESMA de [`Usuario::permissoes_em`], e ela e a mesma
    /// de proposito: quem ja entendeu como a regra da tabela se resolve nao
    /// precisa aprender uma segunda regra para a coluna. Do mais especifico
    /// para o mais geral: a tabela nesta base, `"*"` nesta base, a tabela na
    /// base `"*"`, `"*"` na base `"*"` -- e a primeira que casa **substitui**,
    /// nao intercede.
    pub fn regras_de_coluna(&self, database: &str, tabela: &str) -> &[(String, DireitoDeColuna)] {
        if self.supervisor || tabela.is_empty() {
            return &[];
        }
        for base in [database, "*"] {
            if let Some((_, tabelas)) = self.colunas.iter().find(|(b, _)| b == base) {
                for alvo in [tabela, "*"] {
                    if let Some((_, regras)) = tabelas.iter().find(|(t, _)| t == alvo) {
                        return regras;
                    }
                }
            }
        }
        &[]
    }

    /// Ha alguma regra de coluna nesta tabela desta base?
    ///
    /// E a pergunta que as operacoes que NAO passam pela peneira fazem antes
    /// de recusar: elas devolvem linha por um caminho que ninguem filtra, e
    /// recusar e mais seguro que vazar.
    pub fn tem_regra_de_coluna(&self, database: &str, tabela: &str) -> bool {
        !self.regras_de_coluna(database, tabela).is_empty()
    }

    /// As colunas desta tabela em que este usuario NAO pode a atividade.
    ///
    /// Devolve o nome como o cadastro o escreveu. Quem compara com o esquema
    /// compara sem caixa, porque um `config.json` escrito a mao dificilmente
    /// acerta a caixa de `DataNascimento` -- e uma regra de seguranca que
    /// falha calada por causa de uma maiuscula e pior que uma que nao existe.
    pub fn colunas_negadas(
        &self,
        database: &str,
        tabela: &str,
        atividade: Atividade,
    ) -> Vec<String> {
        self.regras_de_coluna(database, tabela)
            .iter()
            .filter(|(_, d)| !d.pode(atividade))
            .map(|(c, _)| c.clone())
            .collect()
    }

    /// Ficha do usuario, sem a senha. Nunca devolve o hash.
    pub fn ficha(&self) -> Json {
        Json::objeto(vec![
            ("id", Json::de_u64(self.id as u64)),
            ("nome", Json::texto_de(&self.nome)),
            ("login", Json::texto_de(&self.login)),
            ("email", Json::texto_de(&self.email)),
            ("telefone", Json::texto_de(&self.telefone)),
            ("nivel", Json::texto_de(self.nivel.nome())),
            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),
            // Diz que HA chave, nunca qual e. A publica nao e segredo, mas
            // tambem nao ha motivo para espalhar quem usa o que.
            ("exige_chave", Json::Bool(self.chave_publica.is_some())),
            (
                "bases",
                Json::Objeto(
                    self.bases
                        .iter()
                        .map(|(b, p)| (b.clone(), p.para_json()))
                        .collect(),
                ),
            ),
            (
                "tabelas",
                Json::Objeto(
                    self.tabelas
                        .iter()
                        .map(|(b, regras)| {
                            (
                                b.clone(),
                                Json::Objeto(
                                    regras
                                        .iter()
                                        .map(|(t, p)| (t.clone(), p.para_json()))
                                        .collect(),
                                ),
                            )
                        })
                        .collect(),
                ),
            ),
            // As regras de coluna saem PLANAS, e nao aninhadas dentro de
            // `tabelas`: a tela que ja le `tabelas` continua lendo o mesmo
            // objeto de sempre, e quem quiser as colunas pede o campo novo.
            // Aninhar mudaria a forma de um campo que clientes ja consomem.
            (
                "colunas",
                Json::Objeto(
                    self.colunas
                        .iter()
                        .map(|(b, tabelas)| {
                            (
                                b.clone(),
                                Json::Objeto(
                                    tabelas
                                        .iter()
                                        .map(|(t, regras)| {
                                            (
                                                t.clone(),
                                                Json::Objeto(
                                                    regras
                                                        .iter()
                                                        .map(|(c, d)| (c.clone(), d.para_json()))
                                                        .collect(),
                                                ),
                                            )
                                        })
                                        .collect(),
                                ),
                            )
                        })
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

        // supervisor e um admin de todas as bases -- e a forma antiga de
        // dizer a mesma coisa. Mantida, e agora ela ACERTA o nivel, para a
        // ficha nao dizer "leitor" de quem pode tudo.
        let supervisor = j.booleano_ou("supervisor", false);
        let nivel = if supervisor {
            Nivel::Admin
        } else {
            Nivel::de_texto(j.texto_ou("nivel", ""))?
        };

        let chave_publica = match j.campo("chave_publica").and_then(Json::texto) {
            None => None,
            Some(hex) if hex.trim().is_empty() => None,
            Some(hex) => Some(phxsql_core::ed25519::chave_de_hex(hex).ok_or_else(|| {
                PhxError::Esquema(format!(
                    "chave_publica de {login} nao e uma chave Ed25519 (precisa de 64 hexadecimais)"
                ))
            })?),
        };

        let bases = match j.campo("bases") {
            Some(Json::Objeto(pares)) => pares
                .iter()
                .map(|(base, perm)| (base.clone(), Permissoes::de_json(perm)))
                .collect(),
            _ => Vec::new(),
        };

        // `"tabelas"` sai de dentro do objeto da base. Base sem `"tabelas"`
        // nao entra aqui: uma lista vazia e uma lista ausente dariam na mesma
        // no lookup, e a ausente nao ocupa lugar.
        let mut tabelas: Vec<(String, Vec<(String, Permissoes)>)> = Vec::new();
        // E o direito por COLUNA sai de dentro do objeto da TABELA, um nivel
        // abaixo -- pelo mesmo motivo e com a mesma poda: tabela sem
        // `"colunas"` nao entra, e cadastro sem nenhuma deixa a lista vazia,
        // que e o que faz `restringe_colunas` responder `false`.
        let mut colunas: DireitoPorColuna = Vec::new();
        if let Some(Json::Objeto(pares)) = j.campo("bases") {
            for (base, perm) in pares {
                if let Some(Json::Objeto(porta)) = perm.campo("tabelas") {
                    if porta.is_empty() {
                        continue;
                    }
                    tabelas.push((
                        base.clone(),
                        porta
                            .iter()
                            .map(|(t, p)| (t.clone(), Permissoes::de_json(p)))
                            .collect(),
                    ));
                    let mut por_tabela: ColunasDaBase = Vec::new();
                    for (tabela, regra) in porta {
                        let Some(Json::Objeto(cols)) = regra.campo("colunas") else {
                            continue;
                        };
                        if cols.is_empty() {
                            continue;
                        }
                        let mut lista = Vec::with_capacity(cols.len());
                        for (coluna, direito) in cols {
                            // A recusa nomeia usuario, base, tabela e coluna
                            // porque um `config.json` tem dezenas delas: dizer
                            // so "direito desconhecido" manda procurar.
                            let onde = format!("{login}, {base}.{tabela}, coluna {coluna:?}");
                            lista.push((coluna.clone(), DireitoDeColuna::de_json(direito, &onde)?));
                        }
                        por_tabela.push((tabela.clone(), lista));
                    }
                    if !por_tabela.is_empty() {
                        colunas.push((base.clone(), por_tabela));
                    }
                }
            }
        }

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
            supervisor,
            ativo: j.booleano_ou("ativo", true),
            nivel,
            chave_publica,
            bases,
            tabelas,
            colunas,
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

/// O resolvedor de esquema que [`Cadastro::conferir_colunas`] recebe:
/// `(base, tabela)` responde `Ok(Some(colunas))` quando a tabela existe e
/// `Ok(None)` quando a base ou a tabela ainda nao existem. Quem tem o disco
/// o fornece (`servidor.rs`); as provas do cadastro passam um de mentira.
pub type ResolvedorDeEsquema<'a> = &'a mut dyn FnMut(&str, &str) -> Result<Option<Vec<String>>>;

#[cfg(test)]
std::thread_local! {
    static COMPARACOES_DE_LOGIN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Quantas comparacoes de login `Cadastro::por_login` fez nesta thread --
/// so no teste, pedido 529.
///
/// Existe para provar por DENTRO que o custo nao depende de ONDE o login
/// mora (nem de existir): com o defeito, o primeiro da lista para em 1
/// comparacao e quem nao existe varre os N inteiros; com o conserto, os
/// dois pagam exatamente N (mais 1 se ha root). Contador em vez de relogio
/// pela mesma razao do `hash::iteracoes_pagas_nesta_thread`: relogio de
/// teste floca, e a pergunta e "quanto trabalho", nao "quanto tempo".
#[cfg(test)]
pub fn comparacoes_de_login_nesta_thread() -> u64 {
    COMPARACOES_DE_LOGIN.with(std::cell::Cell::get)
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
                u.nivel = Nivel::Admin;
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

    /// Confere cada regra de coluna contra o esquema da tabela que ela cita
    /// -- pedido 235.
    ///
    /// `esquema(base, tabela)` responde `Ok(Some(colunas))` quando a tabela
    /// existe e `Ok(None)` quando a base ou a tabela ainda nao existem.
    ///
    /// # Por que e uma PASSADA depois da carga, e nao um resolvedor dentro de `de_json`
    ///
    /// Porque [`Cadastro::de_json`] e lido por quem nao abre tabela nenhuma:
    /// o `phxsqld --usuarios` lista o cadastro sem subir o servidor, o
    /// `Config::ler` monta o cadastro antes de a raiz de dados existir, e as
    /// provas do cadastro rodam sem disco. Um resolvedor obrigatorio faria os
    /// tres carregar um esquema que nao tem -- ou passar um resolvedor vazio,
    /// que e nao conferir com uma linha a mais. A passada separada deixa o
    /// `de_json` puro e poe a conferencia onde o esquema existe: no arranque
    /// do servidor, com as tabelas em disco, e na porta das tres operacoes de
    /// cadastro, antes de gravar. Sao dois chamadores da MESMA funcao, e nao
    /// duas conferencias.
    ///
    /// # O que recusa, o que avisa e o que passa calado
    ///
    /// * coluna que a tabela nao tem: **recusa**, nomeando usuario, base,
    ///   tabela e coluna, e listando as colunas que existem -- a lista e o
    ///   que faz `salrio` se achar ao lado de `salario`. Ate aqui essa regra
    ///   carregava calada e nao protegia nada: configuracao que nao e lida
    ///   mente;
    /// * base ou tabela que ainda nao existe: **avisa** e aceita. E a mesma
    ///   decisao da chave estrangeira declarada antes da tabela -- ordem
    ///   legitima de modelagem; a regra passa a valer quando a tabela nascer,
    ///   e o `criar_tabela` avisa se ela nascer sem a coluna;
    /// * `"*"` em base ou em tabela: passa sem conferir, porque o curinga nao
    ///   nomeia tabela nenhuma. Conferir contra todas as tabelas recusaria a
    ///   regra legitima «`salario` em qualquer tabela que o tenha».
    ///
    /// A regua de nome e a MESMA da peneira ([`crate::direito_coluna::mesmo_nome`]):
    /// sem caixa e aparada. Uma regua mais dura recusaria `Salario`, que a
    /// peneira aplica; uma mais frouxa deixaria passar o que a peneira nao
    /// acha -- e as duas divergindo e o furo com cara de conferencia.
    ///
    /// Devolve os avisos desta passada; quem chama decide onde eles aparecem
    /// (o arranque imprime, a operacao de cadastro devolve na resposta).
    pub fn conferir_colunas(&self, esquema: ResolvedorDeEsquema<'_>) -> Result<Vec<String>> {
        let mut avisos = Vec::new();
        for u in self.root.iter().chain(self.usuarios.iter()) {
            for (base, tabelas) in &u.colunas {
                if base == "*" {
                    continue;
                }
                for (tabela, regras) in tabelas {
                    if tabela == "*" {
                        continue;
                    }
                    let citadas = || {
                        regras
                            .iter()
                            .map(|(c, _)| format!("{c:?}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    let existentes = match esquema(base, tabela) {
                        Ok(Some(colunas)) => colunas,
                        Ok(None) => {
                            avisos.push(format!(
                                "usuario {}: a regra de coluna em {base}.{tabela} cita uma \
                                 tabela que ainda nao existe (colunas {}). Ela passa a valer \
                                 quando a tabela nascer, e so entao a coluna sera conferida",
                                u.login,
                                citadas()
                            ));
                            continue;
                        }
                        // O erro aqui INFORMA (tabela que nao abre, nome que
                        // nao vale), e por isso viaja inteiro: a conferencia
                        // nao pode derrubar um servidor por uma tabela que o
                        // resto do arranque ainda vai tratar.
                        Err(e) => {
                            avisos.push(format!(
                                "usuario {}: nao consegui conferir a regra de coluna em \
                                 {base}.{tabela} contra o esquema ({e}); a regra ficou como \
                                 esta, sem conferir",
                                u.login
                            ));
                            continue;
                        }
                    };
                    let faltam: Vec<String> = regras
                        .iter()
                        .filter(|(c, _)| {
                            !existentes
                                .iter()
                                .any(|e| crate::direito_coluna::mesmo_nome(e, c))
                        })
                        .map(|(c, _)| format!("{c:?}"))
                        .collect();
                    if !faltam.is_empty() {
                        return Err(PhxError::Esquema(format!(
                            "usuario {}: a regra de coluna em {base}.{tabela} cita {}, e a \
                             tabela nao tem {}. As colunas de {base}.{tabela} sao: {}",
                            u.login,
                            faltam.join(", "),
                            if faltam.len() == 1 {
                                "essa coluna"
                            } else {
                                "essas colunas"
                            },
                            existentes.join(", ")
                        )));
                    }
                }
            }
        }
        Ok(avisos)
    }

    /// Ha alguem cadastrado? Sem cadastro, o servidor cai no token de servico.
    /// Algum usuario exige chave? A pagina de entrada usa isto para decidir
    /// se mostra o campo -- e nao ha segredo nenhum na resposta.
    pub fn alguem_exige_chave(&self) -> bool {
        self.root
            .iter()
            .chain(self.usuarios.iter())
            .any(|u| u.chave_publica.is_some())
    }

    pub fn vazio(&self) -> bool {
        self.root.is_none() && self.usuarios.is_empty()
    }

    /// Acha o usuario pelo login.
    ///
    /// # Custo por TAMANHO do cadastro, e nao por POSICAO -- pedido 529
    ///
    /// O login e unico (`de_json` recusa duplicata na carga, root inclusive),
    /// e por isso um `Iterator::find` que PARA no primeiro que casa devolve
    /// `Some` bem mais rapido para o primeiro da lista do que para quem nao
    /// existe, que precisa varrer os N inteiros. Com cadastro grande isso vira
    /// um relogio que separa "existe" de "nao existe" sem olhar senha
    /// nenhuma -- e o `desafio` e a prova do `op_login` chamam esta funcao
    /// bem ANTES de qualquer PBKDF2 esconder a diferenca (`docs/SEGURANCA.md`
    /// §26.2/§26.6). O SEC mediu em DEBUG, intercalado, n=2.000: com 20.000
    /// usuarios, quem nao existe custava **+363 us** sobre o primeiro da
    /// lista.
    ///
    /// O conserto varre SEMPRE o cadastro inteiro, sem sair cedo em nenhum
    /// casamento -- root, primeiro, ultimo ou ausente pagam o MESMO numero de
    /// comparacoes. Medido em RELEASE por esta frente, antes e depois do
    /// conserto, com login de LARGURA FIXA (`cargo run --release -p
    /// phxsql-server --example custo-do-por-login`, que documenta por que a
    /// largura tem de ser fixa): com 200 usuarios (cadastro realista), a
    /// diferenca entre "nao existe" e o primeiro da lista caiu de 410 ns para
    /// -1 ns (ruido); com 20.000 (o pior caso do SEC), de ~125,7 us para
    /// -357 ns. Numeros completos e a explicacao do artefato de medicao (LTO
    /// levantava a chamada para fora do laco quando a entrada nao variava)
    /// em `docs/SEGURANCA.md` §26.7.
    pub fn por_login(&self, login: &str) -> Option<&Usuario> {
        let mut achado: Option<&Usuario> = None;
        if let Some(r) = &self.root {
            if r.login == login {
                achado = Some(r);
            }
        }
        for u in &self.usuarios {
            // So conta no teste -- pedido 529: contar em producao dobraria o
            // custo de cada comparacao real por um `Cell::set`, e a prova
            // por dentro so precisa existir onde se prova. Ver
            // `comparacoes_de_login_nesta_thread`.
            #[cfg(test)]
            COMPARACOES_DE_LOGIN.with(|c| c.set(c.get() + 1));
            if u.login == login {
                achado = Some(u);
            }
        }
        achado
    }

    /// Confere login e senha. Devolve o usuario so quando os dois batem e a
    /// conta esta ativa.
    ///
    /// # Um PBKDF2 so, do mesmo tamanho para os tres -- pedido 520
    ///
    /// «Nao existe», «inativo» e «senha errada» devolvem a mesma recusa, e
    /// tem de custar o mesmo: senao o relogio responde o que a mensagem
    /// esconde. Por isso ha UMA chamada a `senha::conferir`, e os tres passam
    /// por ela. Quem nao existe confere contra o `senha::hash_de_fachada`, que
    /// tem as `ITERACOES_PADRAO` de todo usuario novo; e o `ativo` so e olhado
    /// DEPOIS da conta.
    ///
    /// O comentario que morava aqui afirmava que os dois primeiros «nao se
    /// distinguem pelo relogio», e o codigo abaixo dele dizia o contrario.
    /// Medido em debug, pela porta de dados: senha errada 2.671 ms; quem nao
    /// existe 25,7 ms (uma fachada de 1.000 iteracoes, fabricada e conferida a
    /// cada login); inativo 0,2 ms (o `ativo &&` pulava o PBKDF2 inteiro).
    ///
    /// O que sobra, e e do desenho: o hash carrega o proprio custo, entao um
    /// hash feito a mao com outra contagem de iteracoes custa outro tempo --
    /// e o `desafio` ja publica essa contagem.
    pub fn autenticar(&self, login: &str, oferecida: &str) -> Option<&Usuario> {
        let achado = self.por_login(login);
        let guardado = achado.map_or(senha::hash_de_fachada(), |u| u.senha_hash.as_str());
        let confere = senha::conferir(oferecida, guardado);
        achado.filter(|u| confere && u.ativo)
    }

    /// Ha alguem ATIVO que possa mexer no cadastro?
    ///
    /// # Por que `e_admin` e nao `supervisor`
    ///
    /// Porque quem manda no cadastro e quem tem `administrar`, e o nivel
    /// `admin` o da tanto quanto a marca de supervisor. Contar so a marca
    /// recusaria a exclusao do ultimo supervisor num servidor que ainda tem
    /// tres administradores -- uma guarda que barra o que nao faz mal ensina
    /// a contorna-la.
    pub fn ha_supervisor_ativo(&self) -> bool {
        self.root
            .iter()
            .chain(self.usuarios.iter())
            .any(|u| u.ativo && u.e_admin())
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

// ===================================================================== edicao
//
// O cadastro deixou de ser SO leitura em 07/09/2026 (pedido 221). Ate aqui ele
// nascia do `config.json` e morria com o processo: criar usuario era editar o
// arquivo a mao e reiniciar o servidor -- e reiniciar um banco para dar acesso
// a alguem e o tipo de custo que faz o administrador dar a conta do vizinho em
// vez de criar uma nova.
//
// # Por que a edicao mexe na ARVORE do arquivo, e nao no `Cadastro`
//
// Porque o `Cadastro` e uma LEITURA do arquivo, e nem tudo o que esta no
// arquivo cabe nele: um campo que este processo nao conhece -- posto ali por
// outra frente, por uma versao mais nova, ou a mao por quem opera -- some no
// caminho de volta. Reserializar o `Cadastro` seria devolver o arquivo de
// alguem podado, calado. Mexer na arvore preserva byte a byte o que nao foi
// pedido, que e a mesma decisao ja tomada em `Config::gravar_campos`.
//
// # A senha vira hash ANTES de a estrutura existir
//
// [`objeto_do_usuario`] e a UNICA porta por onde a senha entra, e ela sai de
// la ja como `senha_hash`. Nao ha um instante em que a arvore que vai ao disco
// -- ou a que volta na resposta -- carregue a senha em claro.

/// A acao pedida sobre o cadastro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    Criar,
    Alterar,
    Excluir,
}

impl Acao {
    fn nome(self) -> &'static str {
        match self {
            Acao::Criar => "usuario_criar",
            Acao::Alterar => "usuario_alterar",
            Acao::Excluir => "usuario_excluir",
        }
    }
}

/// O login e utilizavel? Recusa cedo o que nunca autenticaria.
///
/// # A regra sai do `login`, e nao do gosto de quem escreveu isto
///
/// O pedido de `login` APARA o que chega antes de comparar. Entao um login
/// gravado com espaco na ponta e um login que nao entra nunca: quem o digitar
/// inteiro manda a versao aparada, que nao e igual a gravada. Recusar na
/// criacao custa um erro lido enquanto se cria a conta; aceitar custa uma
/// conta que ninguem consegue usar e ninguem entende por que.
///
/// Espaco NO MEIO continua valendo, e a permissao e deliberada: o `login`
/// aceita, entao recusar aqui seria esta funcao inventando uma regra que o
/// autenticador nao tem.
pub fn login_valido(login: &str) -> Result<()> {
    // O vazio vem antes do espaco: `"   "` e um login que ninguem quis
    // mandar, e a mensagem tem de dizer isso, e nao reclamar da ponta.
    if login.trim().is_empty() {
        return Err(PhxError::Esquema(
            "informe \"login\": o nome pelo qual a pessoa entra".into(),
        ));
    }
    if login != login.trim() {
        return Err(PhxError::Esquema(format!(
            "o login {login:?} comeca ou termina com espaco, e o `login` apara \
             antes de comparar: esta conta nunca entraria"
        )));
    }
    if let Some(c) = login.chars().find(|c| c.is_control()) {
        return Err(PhxError::Esquema(format!(
            "o login {login:?} tem um caractere de controle ({:?}), que nao \
             sobrevive ao log nem ao terminal",
            c
        )));
    }
    if login.chars().count() > LOGIN_MAX {
        return Err(PhxError::Esquema(format!(
            "o login tem {} caracteres e o teto e {LOGIN_MAX}",
            login.chars().count()
        )));
    }
    Ok(())
}

/// Teto do login. Nao ha limite tecnico -- ha limite de tela e de log.
pub const LOGIN_MAX: usize = 64;

/// Os campos que o pedido pode trazer, alem de `login` e `senha`.
///
/// Lista de PERMISSAO, e nao de recusa: campo novo que alguem mandar nasce
/// RECUSADO ate ser declarado aqui. E o mesmo principio do portao que nega
/// operacao desconhecida -- e, aqui, o que impede um pedido de escrever
/// `senha_hash` cru, contornando o PBKDF2 desta casa.
const CAMPOS_DO_USUARIO: &[&str] = &[
    "id",
    "nome",
    "email",
    "telefone",
    "nivel",
    "supervisor",
    "ativo",
    "bases",
    "chave_publica",
];

/// Muda a lista `usuarios` da arvore do `config.json`.
///
/// Devolve o login alcancado. Nao grava nada: quem grava e
/// [`crate::config::Config::gravar_usuarios`], e a separacao e de proposito --
/// as regras de cadastro se provam sem disco nenhum.
///
/// `quem` e a sessao que pediu; `None` e o token de servico, que nao tem conta
/// para proteger de si mesma.
pub fn aplicar_na_arvore(
    arvore: &mut Json,
    acao: Acao,
    p: &Json,
    quem: Option<&Usuario>,
) -> Result<String> {
    let login = p.texto_ou("login", p.texto_ou("usuario", "")).to_string();
    login_valido(&login)?;

    // O root nao entra por aqui, e a recusa e uma decisao.
    //
    // Ele e a ultima porta de entrada quando o resto do cadastro sai errado --
    // inclusive quando quem saiu errado foi uma destas tres operacoes. Uma op
    // de protocolo que pudesse trocar a senha do root, ou desliga-lo, seria a
    // porta e a chave no mesmo molho.
    if arvore
        .campo("root")
        .map(|r| r.texto_ou("login", "root") == login)
        .unwrap_or(false)
        || login == "root" && arvore.campo("root").is_some()
    {
        return Err(PhxError::Autorizacao(format!(
            "o root nao se muda pelo protocolo: ele e a porta de entrada de \
             quando o cadastro sai errado. Edite o config.json para mexer em {login:?}"
        )));
    }

    // So supervisor faz supervisor. Um `admin` com poder em UMA base pediria
    // `supervisor: true` e sairia podendo tudo em TODAS -- escalada por
    // cadastro. Quem ja e supervisor nao escala nada criando outro.
    let pede_supervisor = p.booleano_ou("supervisor", false);
    if pede_supervisor && !quem.map(|u| u.supervisor).unwrap_or(true) {
        return Err(PhxError::Autorizacao(
            "so um supervisor cria ou promove supervisor: administrar uma base \
             nao da o poder de criar quem manda em todas"
                .into(),
        ));
    }

    let mut lista: Vec<Json> = match arvore.campo("usuarios") {
        Some(Json::Lista(itens)) => itens.clone(),
        _ => Vec::new(),
    };
    let posicao = lista
        .iter()
        .position(|u| u.texto_ou("login", "").trim() == login);

    match acao {
        Acao::Criar => {
            if posicao.is_some() {
                return Err(PhxError::Esquema(format!(
                    "ja ha um usuario com o login {login:?}"
                )));
            }
            lista.push(objeto_do_usuario(&login, p, None)?);
        }
        Acao::Alterar => {
            let i = posicao.ok_or_else(|| nao_existe(&login))?;
            lista[i] = objeto_do_usuario(&login, p, Some(&lista[i]))?;
        }
        Acao::Excluir => {
            let i = posicao.ok_or_else(|| nao_existe(&login))?;
            lista.remove(i);
        }
    }
    arvore.definir("usuarios", Json::Lista(lista));

    // As duas guardas que so se conferem no RESULTADO, e nao no pedido.
    //
    // Conferir a intencao nao basta: «tirar o supervisor de fulano» so e o
    // ultimo supervisor DEPOIS de saber quem sobrou, e quem sobrou depende do
    // arquivo inteiro. Ler o cadastro de volta e o que transforma a pergunta
    // «isto parece perigoso?» em «isto DEIXOU o servidor sem dono?».
    let novo = Cadastro::de_json(arvore)?;
    if !novo.ha_supervisor_ativo() {
        return Err(PhxError::Autorizacao(format!(
            "{}: isto deixaria o servidor sem nenhum supervisor ativo, e ai \
             ninguem mais poderia mexer no cadastro -- nem para desfazer",
            acao.nome()
        )));
    }
    if let Some(eu) = quem {
        let continuo = novo
            .por_login(&eu.login)
            .is_some_and(|u| u.ativo && u.pode_em("", "", Atividade::Administrar));
        if !continuo {
            return Err(PhxError::Autorizacao(format!(
                "{}: isto tiraria de {:?} -- a sua propria conta -- o poder de \
                 administrar. Peca a outro administrador",
                acao.nome(),
                eu.login
            )));
        }
    }
    Ok(login)
}

fn nao_existe(login: &str) -> PhxError {
    PhxError::NaoEncontrado(format!(
        "nao ha usuario com o login {login:?}; a lista esta em `usuarios`"
    ))
}

/// O objeto do usuario como ele vai para o `config.json`.
///
/// `anterior` e o objeto que ja estava no arquivo, quando havia um: os campos
/// que o pedido NAO traz ficam como estavam, e os que este processo nao
/// conhece ficam tambem. Alterar o telefone de alguem nao pode apagar a chave
/// publica dele nem um campo que outra versao gravou.
fn objeto_do_usuario(login: &str, p: &Json, anterior: Option<&Json>) -> Result<Json> {
    let mut pares: Vec<(String, Json)> = match anterior {
        Some(Json::Objeto(velhos)) => velhos.clone(),
        _ => Vec::new(),
    };
    let mut por = |nome: &str, valor: Json| match pares.iter_mut().find(|(k, _)| k == nome) {
        Some((_, v)) => *v = valor,
        None => pares.push((nome.to_string(), valor)),
    };
    por("login", Json::texto_de(login));

    for campo in CAMPOS_DO_USUARIO {
        if let Some(v) = p.campo(campo) {
            por(campo, v.clone());
        }
    }

    // A SENHA. Entra em claro e sai hash, e este e o unico ponto do caminho
    // em que ela existe -- por isso o `senha_hash` cru e recusado ao lado: um
    // pedido que trouxesse o hash pronto escolheria as proprias iteracoes, e
    // «PBKDF2 com 1 volta» tem a mesma cara de «PBKDF2 com 210.000».
    if p.campo("senha_hash").is_some() {
        return Err(PhxError::Autorizacao(
            "\"senha_hash\" nao se manda pelo protocolo: mande \"senha\" e o \
             servidor deriva o hash. Hash pronto escolheria o proprio custo"
                .into(),
        ));
    }
    match p.campo("senha").and_then(Json::texto) {
        Some(clara) if !clara.is_empty() => {
            // O teto ANTES do PBKDF2 (pedido 521): senha que o login recusaria
            // nao nasce, e o `CREATE USER` de 1 MiB deixa de ser uma conta.
            senha::caber_no_teto(clara)?;
            por("senha_hash", Json::texto_de(senha::cifrar(clara)));
            // A senha em texto puro que o formato ainda aceita sai JUNTO: um
            // usuario que a tinha e trocou de senha nao pode continuar com a
            // velha em claro no arquivo, sem ninguem notar.
            pares.retain(|(k, _)| k != "senha");
        }
        Some(_) => {
            return Err(PhxError::Esquema(
                "\"senha\" vazia: mande a senha, ou omita o campo para nao mexer nela".into(),
            ))
        }
        None if anterior.is_some() => {}
        None => {
            return Err(PhxError::Esquema(
                "informe \"senha\": um usuario sem senha nao entra".into(),
            ))
        }
    }
    Ok(Json::Objeto(pares))
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

    // ------------------------------------------------------- pedido 520
    //
    // O relogio nao pode dizer quem existe. As tres provas contam por DENTRO
    // quantas iteracoes de PBKDF2 o `autenticar` pagou, em vez de medir
    // tempo: separar 64 de 210.000 pelo relogio flocaria, e o contador nao.

    /// A ana ativa e o ze inativo, os dois com o hash de 64 iteracoes.
    fn cadastro_do_520() -> Cadastro {
        cadastro(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}"}},
                            {{"login":"ze","senha_hash":"{}","ativo":false}}]}}"#,
            hash_rapido("senha-da-ana"),
            hash_rapido("senha-do-ze")
        ))
    }

    /// Entrou? E quantas iteracoes de PBKDF2 esta thread pagou para decidir.
    fn iteracoes_do_login(c: &Cadastro, login: &str, oferecida: &str) -> (bool, u64) {
        let antes = phxsql_core::hash::iteracoes_pagas_nesta_thread();
        let entrou = c.autenticar(login, oferecida).is_some();
        (
            entrou,
            phxsql_core::hash::iteracoes_pagas_nesta_thread() - antes,
        )
    }

    /// **O defeito do 520.** Quem nao existe paga as `ITERACOES_PADRAO` com
    /// que todo usuario novo nasce -- as 210.000 de quem existe.
    ///
    /// Vermelho medido com a fachada de antes (`cifrar_com("nao-existe",
    /// 1_000)` e o `conferir` contra ela): 2.000 iteracoes pagas, contra as
    /// 210.000 esperadas.
    #[test]
    fn quem_nao_existe_paga_o_pbkdf2_de_um_usuario_novo() {
        let c = cadastro_do_520();
        let (entrou, pagas) = iteracoes_do_login(&c, "nao_existe", "qualquer-senha");
        assert!(!entrou);
        assert_eq!(
            pagas,
            u64::from(senha::ITERACOES_PADRAO),
            "o login de quem nao existe pagou outro PBKDF2: o relogio diz quem existe"
        );
    }

    /// **O irmao do 520 que o SEC nao listou: o inativo.** O `ativo &&` de
    /// antes curto-circuitava a conta, e o inativo respondia sem PBKDF2
    /// nenhum -- medido em debug, 0,2 ms contra 2.671 ms da senha errada.
    /// Consertar so a fachada teria deixado o inativo como o UNICO caminho
    /// rapido, e o relogio passaria a dizer «existe, e esta desligado».
    ///
    /// Vermelho medido com o curto-circuito reposto: 0 iteracoes pagas,
    /// contra as 64 do hash dele.
    #[test]
    fn quem_esta_inativo_paga_o_pbkdf2_do_proprio_hash() {
        let c = cadastro_do_520();
        for oferecida in ["senha-do-ze", "senha-errada"] {
            let (entrou, pagas) = iteracoes_do_login(&c, "ze", oferecida);
            assert!(!entrou, "inativo entrou com {oferecida:?}");
            assert_eq!(pagas, 64, "o inativo pulou a conta com {oferecida:?}");
        }
    }

    /// O caminho de referencia: quem existe paga o hash dele, errando ou
    /// acertando, e so a senha certa entra.
    #[test]
    fn quem_existe_paga_o_pbkdf2_do_proprio_hash() {
        let c = cadastro_do_520();
        assert_eq!(iteracoes_do_login(&c, "ana", "senha-errada"), (false, 64));
        assert_eq!(iteracoes_do_login(&c, "ana", "senha-da-ana"), (true, 64));
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

    /// O `Debug` do usuario tambem nao devolve o hash.
    ///
    /// Irma da de cima, e a saida que faltava: a `ficha` cumpria a promessa e o
    /// `derive(Debug)` a desfazia. O hash NAO e um resumo inofensivo -- e dele
    /// que sai a chave do desafio-resposta, entao quem o tem se autentica como
    /// o usuario sem nunca saber a senha.
    ///
    /// Confere as duas formas de escrever, como a `a_privada_do_fio_nunca_sai`.
    #[test]
    fn o_debug_do_usuario_nunca_mostra_o_hash() {
        let h = hash_rapido("segredo");
        let txt = format!(r#"{{"usuarios":[{{"login":"ana","senha_hash":"{h}"}}]}}"#);
        let c = cadastro(&txt);
        let u = c.por_login("ana").unwrap();
        assert_eq!(u.senha_hash, h, "o hash nem chegou a ser lido");

        for texto in [format!("{:?}", u), format!("{u:?}")] {
            assert!(!texto.contains(&h), "o hash vazou no Debug: {texto}");
            assert!(!texto.contains("pbkdf2"), "o algoritmo vazou: {texto}");
            assert!(texto.contains("ana"), "o Debug perdeu o login: {texto}");
        }
        // E o cadastro inteiro, que e o que alguem realmente imprimiria.
        assert!(!format!("{c:?}").contains(&h), "o hash vazou pelo cadastro");
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
        assert_eq!(Atividade::da_operacao("desafio"), None);
        assert_eq!(
            Atividade::da_operacao("bloqueios"),
            Some(Atividade::Administrar)
        );
        assert_eq!(
            Atividade::da_operacao("desbloquear"),
            Some(Atividade::Administrar)
        );
        // Operacao desconhecida exige o maior poder, em vez de passar batido.
        assert_eq!(
            Atividade::da_operacao("op_que_nao_existe"),
            Some(Atividade::Administrar)
        );
    }
    #[test]
    fn a_memoria_pede_leitura_e_o_backup_pede_administrar() {
        // Consultar em memoria nao pode exigir mais poder do que ler do disco:
        // e o mesmo dado. Ja o backup e conta de administrador.
        for op in [
            "memoria_carregar",
            "memoria",
            "SelectMemory",
            "selecionar_memoria",
        ] {
            assert_eq!(Atividade::da_operacao(op), Some(Atividade::Ler), "{op}");
        }
        for op in ["backup", "conferir_backup", "memoria_liberar"] {
            assert_eq!(
                Atividade::da_operacao(op),
                Some(Atividade::Administrar),
                "{op}"
            );
        }
        assert_eq!(Atividade::da_operacao("sair"), None);
    }
    /// **Operação catalogada que esqueceu a linha do poder vira administrador
    /// em silêncio.**
    ///
    /// O `_ => Administrar` fecha a porta para o desconhecido, e isso está
    /// certo. O que ele não distingue é a operação que ESTÁ no catálogo e
    /// esqueceu a sua linha: ela nasce exigindo o maior poder, ninguém é
    /// avisado, e o sintoma chega como «não consigo usar isto» de quem podia.
    /// Aconteceu com o `procurar_texto` nesta mesma rodada.
    ///
    /// A conferência lê o FONTE e exige que cada nome do catálogo apareça
    /// escrito ali — como o conferidor da `fase` da telemetria faz, e pelo
    /// mesmo motivo: a função sozinha não sabe dizer se respondeu por regra ou
    /// por omissão.
    #[test]
    fn toda_operacao_do_catalogo_declara_o_poder_que_pede() {
        const FONTE: &str = include_str!("usuarios.rs");
        let corpo = {
            let de = FONTE
                .find("pub fn da_operacao")
                .expect("o conferidor nao achou a funcao: leitor quebrado passa por engano");
            let ate = FONTE[de..]
                .find("\n    }\n")
                .expect("o conferidor nao achou o fim da funcao");
            &FONTE[de..de + ate]
        };
        // Recusa passar vendo zero: leitor quebrado tem de reprovar, e nao
        // aprovar tudo.
        assert!(
            corpo.contains("\"buscar\""),
            "o conferidor nao esta lendo o corpo certo"
        );
        // Os APELIDOS entram: o cliente manda `join` e o portao classifica o
        // que o cliente mandou. Um apelido que existisse so no catalogo
        // passaria pelo conferidor e cairia no `_` no ar.
        let faltando: Vec<&str> = crate::catalogo::OPERACOES
            .iter()
            .flat_map(|o| o.nomes())
            .filter(|n| !corpo.contains(&format!("\"{n}\"")))
            .collect();
        assert!(
            faltando.is_empty(),
            "estas operacoes estao no catalogo e nao declaram poder em \
             `Atividade::da_operacao` -- vao virar ADMINISTRAR em silencio: {faltando:?}"
        );
    }

    #[test]
    fn cada_nivel_contem_o_anterior() {
        let nenhum = Nivel::Nenhum.permissoes();
        let leitor = Nivel::Leitor.permissoes();
        let operador = Nivel::Operador.permissoes();
        let dono = Nivel::Dono.permissoes();
        let admin = Nivel::Admin.permissoes();

        for a in Atividade::TODAS {
            assert!(!nenhum.pode(a), "nenhum deu {}", a.nome());
            if leitor.pode(a) {
                assert!(operador.pode(a), "operador perdeu {}", a.nome());
            }
            if operador.pode(a) {
                assert!(dono.pode(a), "dono perdeu {}", a.nome());
            }
            if dono.pode(a) {
                assert!(admin.pode(a), "admin perdeu {}", a.nome());
            }
        }

        // E cada um acrescenta alguma coisa de verdade.
        assert!(!leitor.inserir && operador.inserir);
        assert!(!operador.criar && dono.criar);
        assert!(!dono.administrar && admin.administrar);
        // Leitor le, e so.
        assert!(leitor.ler && leitor.diario && leitor.verificar);
        assert!(!leitor.excluir && !leitor.reindexar && !leitor.replicar);
    }

    #[test]
    fn o_nivel_vale_onde_nao_ha_regra_de_base() {
        let txt = format!(
            r#"{{"usuarios":[{{
                 "login":"ana","senha_hash":"{}","nivel":"operador",
                 "bases":{{"Financeiro":{{}}}}
               }}]}}"#,
            hash_rapido("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let ana = c.por_login("ana").unwrap();
        assert_eq!(ana.nivel, Nivel::Operador);
        // Onde nao ha regra, vale o nivel.
        assert!(ana.pode("Comercial", Atividade::Inserir));
        assert!(ana.pode("Comercial", Atividade::Ler));
        assert!(!ana.pode("Comercial", Atividade::Criar));
        // Onde HA regra, a regra manda -- mesmo para tirar poder.
        assert!(!ana.pode("Financeiro", Atividade::Ler));
        assert!(!ana.pode("Financeiro", Atividade::Inserir));
    }

    #[test]
    fn sem_nivel_o_padrao_nega_tudo() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"ze","senha_hash":"{}"}}]}}"#,
            hash_rapido("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let ze = c.por_login("ze").unwrap();
        assert_eq!(ze.nivel, Nivel::Nenhum);
        assert!(!ze.e_admin());
        // Nada. Config que nao diz nivel nao ganha poder nenhum de brinde --
        // e o que faz esta mudanca nao alterar nenhum config que ja existe.
        for a in Atividade::TODAS {
            assert!(!ze.pode("Qualquer", a), "sem nivel deu {}", a.nome());
        }
    }

    #[test]
    fn nivel_admin_e_supervisor_dizem_a_mesma_coisa() {
        let txt = format!(
            r#"{{"usuarios":[
                 {{"login":"a","senha_hash":"{h}","nivel":"admin"}},
                 {{"login":"b","senha_hash":"{h}","supervisor":true}}
               ]}}"#,
            h = hash_rapido("x")
        );
        let c = Cadastro::de_json(&Json::analisar(&txt).unwrap()).unwrap();
        let a = c.por_login("a").unwrap();
        let b = c.por_login("b").unwrap();
        assert!(a.e_admin() && b.e_admin());
        // A ficha do supervisor nao pode dizer "leitor" de quem pode tudo.
        assert_eq!(b.nivel, Nivel::Admin);
        for at in Atividade::TODAS {
            assert_eq!(
                a.pode("Comercial", at),
                b.pode("Comercial", at),
                "divergiram em {}",
                at.nome()
            );
        }
    }

    #[test]
    fn nivel_desconhecido_nao_sobe() {
        let txt = format!(
            r#"{{"usuarios":[{{"login":"x","senha_hash":"{}","nivel":"chefao"}}]}}"#,
            hash_rapido("x")
        );
        assert!(Cadastro::de_json(&Json::analisar(&txt).unwrap()).is_err());
    }

    /* ------------------------------------------------- a edicao do cadastro
    Estas provas nao tocam disco: `aplicar_na_arvore` mexe na ARVORE, e
    quem grava e o `config.rs`. A separacao existe justamente para as
    regras de cadastro se provarem sem arquivo nenhum. */

    fn arvore(txt: &str) -> Json {
        Json::analisar(txt).unwrap()
    }

    fn supervisora() -> Usuario {
        Usuario {
            id: 9,
            nome: "Ana".into(),
            login: "ana".into(),
            senha_hash: hash_rapido("x"),
            email: String::new(),
            telefone: String::new(),
            supervisor: true,
            ativo: true,
            nivel: Nivel::Admin,
            chave_publica: None,
            bases: Vec::new(),
            tabelas: Vec::new(),
            colunas: Vec::new(),
        }
    }

    /// **A armadilha do formato.** O `config.json` ainda aceita `"senha"` em
    /// texto puro, com aviso -- e um cadastro antigo pode ter uma. Trocar a
    /// senha dessa pessoa tem de LEVAR A VELHA JUNTO: um arquivo que fica com
    /// `senha_hash` novo e `senha` velha ao lado guarda a senha antiga em
    /// claro para sempre, e ninguem olha de novo.
    #[test]
    fn trocar_a_senha_leva_junto_a_que_estava_em_texto_puro() {
        let mut a = arvore(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}","supervisor":true}},
                             {{"login":"velho","senha":"12345678"}}]}}"#,
            hash_rapido("x")
        ));
        aplicar_na_arvore(
            &mut a,
            Acao::Alterar,
            &arvore(r#"{"login":"velho","senha":"a-senha-nova"}"#),
            Some(&supervisora()),
        )
        .unwrap();
        let texto = a.escrever();
        assert!(
            !texto.contains("12345678"),
            "a senha velha em texto puro ficou no arquivo: {texto}"
        );
        assert!(
            !texto.contains("a-senha-nova"),
            "a senha nova entrou em claro: {texto}"
        );
        assert!(texto.contains("pbkdf2-sha256$"), "{texto}");
    }

    /// A regra sai do `login`, que APARA antes de comparar: o que nunca
    /// autenticaria e recusado na criacao.
    #[test]
    fn o_login_se_valida_pelo_que_o_login_aceita() {
        assert!(login_valido("carlos").is_ok());
        assert!(
            login_valido("carlos da silva").is_ok(),
            "espaco NO MEIO o `login` aceita, entao aqui tambem"
        );
        assert!(login_valido("").is_err());
        assert!(login_valido("   ").is_err());
        assert!(login_valido(" carlos").is_err());
        assert!(login_valido("carlos ").is_err());
        assert!(login_valido("car\nlos").is_err());
        assert!(login_valido(&"c".repeat(LOGIN_MAX)).is_ok());
        assert!(login_valido(&"c".repeat(LOGIN_MAX + 1)).is_err());
    }

    /// O `chave_publica` de quem usa segundo fator nao pode sumir porque
    /// alguem mudou o telefone dele.
    #[test]
    fn alterar_preserva_a_chave_publica_e_o_que_o_processo_nao_conhece() {
        let chave = "a".repeat(64);
        let mut a = arvore(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}","supervisor":true}},
                {{"login":"bia","senha_hash":"{}","chave_publica":"{chave}",
                  "campo_de_outra_versao":42}}]}}"#,
            hash_rapido("x"),
            hash_rapido("y")
        ));
        aplicar_na_arvore(
            &mut a,
            Acao::Alterar,
            &arvore(r#"{"login":"bia","telefone":"+55 47 90000-0000"}"#),
            Some(&supervisora()),
        )
        .unwrap();
        let c = Cadastro::de_json(&a).unwrap();
        let bia = c.por_login("bia").unwrap();
        assert!(bia.chave_publica.is_some(), "a chave publica sumiu");
        assert_eq!(bia.telefone, "+55 47 90000-0000");
        assert!(a.escrever().contains("campo_de_outra_versao"));
    }

    /// Guarda pela consequencia, e nao pela intencao: o que se confere e o
    /// cadastro que SOBROU, e nao o que o pedido parecia querer.
    #[test]
    fn o_ultimo_administrador_nao_se_apaga() {
        let mut a = arvore(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}","supervisor":true}}]}}"#,
            hash_rapido("x")
        ));
        // Sem `quem` -- token de servico --, para a guarda que pega ser a do
        // ultimo administrador, e nao a da propria conta.
        let e = aplicar_na_arvore(&mut a, Acao::Excluir, &arvore(r#"{"login":"ana"}"#), None)
            .unwrap_err()
            .to_string();
        assert!(e.contains("supervisor ativo"), "{e}");
    }

    /// E ela nao barra o que nao faz mal: com dois, um sai.
    #[test]
    fn com_dois_administradores_um_sai() {
        let mut a = arvore(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}","supervisor":true}},
                             {{"login":"bia","senha_hash":"{}","nivel":"admin"}}]}}"#,
            hash_rapido("x"),
            hash_rapido("y")
        ));
        aplicar_na_arvore(
            &mut a,
            Acao::Excluir,
            &arvore(r#"{"login":"bia"}"#),
            Some(&supervisora()),
        )
        .unwrap();
        let c = Cadastro::de_json(&a).unwrap();
        assert!(c.por_login("bia").is_none());
        assert!(c.ha_supervisor_ativo());
    }

    #[test]
    fn os_apelidos_de_nivel_valem() {
        assert_eq!(Nivel::de_texto("ADMIN").unwrap(), Nivel::Admin);
        assert_eq!(Nivel::de_texto(" dba ").unwrap(), Nivel::Admin);
        assert_eq!(Nivel::de_texto("consulta").unwrap(), Nivel::Leitor);
        assert_eq!(Nivel::de_texto("").unwrap(), Nivel::Nenhum);
        assert_eq!(Nivel::de_texto("nenhum").unwrap(), Nivel::Nenhum);
        assert_eq!(Nivel::de_texto("leitor").unwrap(), Nivel::Leitor);
        assert_eq!(Nivel::de_texto("owner").unwrap(), Nivel::Dono);
    }

    // ------------------------ a conferencia da coluna contra o esquema (235)

    /// O cadastro da ana com UMA regra de coluna em `<base>.<tabela>`.
    fn com_regra_em(base: &str, tabela: &str, coluna: &str) -> Cadastro {
        cadastro(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}",
                "bases":{{"{base}":{{"ler":true,"tabelas":{{"{tabela}":{{"ler":true,
                "colunas":{{"{coluna}":{{"ler":false}}}}}}}}}}}}}}]}}"#,
            hash_rapido("x")
        ))
    }

    /// O esquema de `b.folha`: id, nome, salario. Nenhuma outra tabela existe.
    fn so_a_folha(base: &str, tabela: &str) -> Result<Option<Vec<String>>> {
        Ok((base == "b" && tabela == "folha").then(|| {
            ["id", "nome", "salario"]
                .iter()
                .map(|c| c.to_string())
                .collect()
        }))
    }

    /// **Comportamento velho.** Cadastro sem `"colunas"` nao pergunta esquema
    /// nenhum -- o resolvedor que panica prova que nem e chamado. E o que faz
    /// a conferencia custar zero para todo `config.json` de hoje.
    #[test]
    fn sem_colunas_a_conferencia_nao_pergunta_nada() {
        let c = cadastro(&format!(
            r#"{{"usuarios":[{{"login":"ana","senha_hash":"{}",
                "bases":{{"b":{{"ler":true,"tabelas":{{"folha":{{"ler":true}}}}}}}}}}]}}"#,
            hash_rapido("x")
        ));
        let avisos = c
            .conferir_colunas(&mut |b, t| panic!("perguntou por {b}.{t} sem regra de coluna"))
            .unwrap();
        assert!(avisos.is_empty());
    }

    /// **Comportamento velho.** A coluna existe: passa calado, sem aviso.
    #[test]
    fn coluna_que_existe_passa_sem_aviso() {
        let avisos = com_regra_em("b", "folha", "salario")
            .conferir_colunas(&mut so_a_folha)
            .unwrap();
        assert!(avisos.is_empty(), "{avisos:?}");
    }

    /// **O defeito do pedido 235.** `salrio` carregava calado e `salario`
    /// continuava sem regra. Agora recusa nomeando usuario, base, tabela e
    /// coluna -- e listando as colunas que existem, para o erro de digitacao
    /// se achar.
    #[test]
    fn coluna_com_erro_de_digitacao_recusa_nomeando() {
        let e = com_regra_em("b", "folha", "salrio")
            .conferir_colunas(&mut so_a_folha)
            .unwrap_err();
        let texto = e.to_string();
        eprintln!("a recusa: {texto}");
        for pedaco in ["ana", "b.folha", "\"salrio\"", "id, nome, salario"] {
            assert!(
                texto.contains(pedaco),
                "a recusa nao diz {pedaco:?}: {texto}"
            );
        }
    }

    /// Tabela que ainda nao existe: aceita, com um aviso que nomeia usuario,
    /// tabela e as colunas citadas -- a mesma decisao da chave estrangeira
    /// declarada antes da tabela.
    #[test]
    fn tabela_que_ainda_nao_existe_aceita_com_aviso() {
        let avisos = com_regra_em("b", "bonus", "valor")
            .conferir_colunas(&mut so_a_folha)
            .unwrap();
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(
            avisos[0].contains("ana")
                && avisos[0].contains("b.bonus")
                && avisos[0].contains("\"valor\""),
            "{avisos:?}"
        );
    }

    /// A regua e a da peneira: sem caixa e aparada. `Salario` nao e erro de
    /// digitacao -- a peneira o aplica, e a conferencia nao pode recusar o que
    /// a peneira aceita.
    #[test]
    fn a_caixa_nao_e_erro_de_digitacao() {
        let avisos = com_regra_em("b", "folha", "Salario")
            .conferir_colunas(&mut so_a_folha)
            .unwrap();
        assert!(avisos.is_empty(), "{avisos:?}");
    }

    /// O curinga nao nomeia tabela: nem se confere, nem se avisa.
    #[test]
    fn curinga_de_base_ou_de_tabela_nao_se_confere() {
        for (base, tabela) in [("*", "folha"), ("b", "*"), ("*", "*")] {
            let avisos = com_regra_em(base, tabela, "salrio")
                .conferir_colunas(&mut |b, t| panic!("perguntou por {b}.{t}, que e curinga"))
                .unwrap();
            assert!(avisos.is_empty(), "{base}.{tabela}: {avisos:?}");
        }
    }

    /// Tabela que nao abre nao derruba a carga: vira aviso com o motivo, e a
    /// regra fica como esta. O resto do arranque e que trata a tabela.
    #[test]
    fn tabela_que_nao_abre_vira_aviso_e_nao_recusa() {
        let avisos = com_regra_em("b", "folha", "salrio")
            .conferir_colunas(&mut |_, _| Err(PhxError::Corrompido("folha.reg truncado".into())))
            .unwrap();
        assert_eq!(avisos.len(), 1, "{avisos:?}");
        assert!(avisos[0].contains("folha.reg truncado"), "{avisos:?}");
    }

    // -------------------------------------------------- pedido 529, o teto

    /// Um cadastro com `n` usuarios, todos com o MESMO hash valido -- o
    /// conteudo nao importa para o `por_login`, e gerar um hash de verdade
    /// por usuario pagaria o PBKDF2 de `n` contas so para montar o cenario.
    fn cadastro_com_n_usuarios(n: usize) -> Cadastro {
        let mut usuarios = String::with_capacity(n * 56);
        for i in 0..n {
            if i > 0 {
                usuarios.push(',');
            }
            usuarios.push_str(&format!(
                r#"{{"login":"u{i}","id":{},"senha_hash":"pbkdf2-sha256$1000$00$00"}}"#,
                i as u32 + 1
            ));
        }
        cadastro(&format!(r#"{{"usuarios":[{usuarios}]}}"#))
    }

    /// **Prova por dentro: `por_login` custa o MESMO numero de comparacoes
    /// para o primeiro da lista, o ultimo e quem nao existe -- pedido 529.**
    ///
    /// Com o defeito (o `find` que para cedo), o primeiro custaria 1
    /// comparacao e "nao existe" custaria as 500 inteiras: um relogio que
    /// separa quem existe de quem nao existe sem olhar senha nenhuma. Contador
    /// em vez de relogio pela mesma razao do pedido 520: relogio de teste
    /// floca.
    #[test]
    fn por_login_faz_o_mesmo_numero_de_comparacoes_para_qualquer_login() {
        let c = cadastro_com_n_usuarios(500);

        let antes = comparacoes_de_login_nesta_thread();
        assert!(c.por_login("u0").is_some());
        let do_primeiro = comparacoes_de_login_nesta_thread() - antes;

        let antes = comparacoes_de_login_nesta_thread();
        assert!(c.por_login("u499").is_some());
        let do_ultimo = comparacoes_de_login_nesta_thread() - antes;

        let antes = comparacoes_de_login_nesta_thread();
        assert!(c.por_login("nao-existe-nenhum").is_none());
        let de_quem_nao_existe = comparacoes_de_login_nesta_thread() - antes;

        assert_eq!(
            do_primeiro, 500,
            "o primeiro da lista parou cedo em vez de varrer os 500"
        );
        assert_eq!(do_primeiro, do_ultimo, "primeiro e ultimo divergem");
        assert_eq!(
            do_primeiro, de_quem_nao_existe,
            "quem nao existe custa um numero diferente de comparacoes: \
             e exatamente o oraculo de tempo que o pedido 529 fecha"
        );
    }

    /// **Comportamento VELHO: o resultado continua o mesmo**, so o CUSTO
    /// mudou. Root, primeiro, ultimo e quem nao existe respondem exatamente
    /// o que respondiam antes.
    #[test]
    fn por_login_continua_achando_quem_existe_e_recusando_quem_nao_existe() {
        let txt = format!(
            r#"{{"root":{{"login":"root","senha_hash":"{}"}},
                 "usuarios":[{{"login":"ana","id":1,"senha_hash":"{}"}}]}}"#,
            hash_rapido("r"),
            hash_rapido("a")
        );
        let c = cadastro(&txt);
        assert_eq!(c.por_login("root").unwrap().login, "root");
        assert_eq!(c.por_login("ana").unwrap().login, "ana");
        assert!(c.por_login("fulano").is_none());
    }
}
