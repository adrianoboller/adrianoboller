//! Servidor TCP do PhxSql.
//!
//! Protocolo JSON Lines: uma linha JSON por pedido, uma linha JSON por
//! resposta, UTF-8, terminadas em `\n`. A conexao aceita varios pedidos
//! seguidos e cada um vira uma entrada no log de acessos.
//!
//! ```text
//! -> {"token":"...","op":"ping"}
//! <- {"ok":true,"op":"ping","resultado":{"phxsql":"0.1.0"},"ms":0}
//! ```
//!
//! # Concorrencia
//!
//! O motor de armazenamento ainda nao tem travas de arquivo nem de registro,
//! entao TODO acesso a dados passa por um mutex unico: as conexoes sao
//! aceitas em paralelo, mas as operacoes se enfileiram. E lento sob carga e e
//! correto -- o contrario seria rapido e corrompido. Travas finas entram junto
//! com as transacoes.
//! DIVIDA: #164 nao ha trava de arquivo nem de registro -- toda operacao de dados se enfileira no mutex global, e as travas finas continuam por fazer

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::expressao::Expressao;
use phxsql_core::fio::{Canal, Recebido, TETO_DO_APERTO, TETO_DO_REGISTRO};
use phxsql_core::json::Json;
use phxsql_core::semaforo::{Permissao, Semaforo};
use phxsql_store::catalogo::{Aberta, Instancia, PorSincronizar, Raiz};
use phxsql_store::leitura::{DiarioLegivel, Legivel, TabelaLeitura};
use phxsql_store::log::Operacao;
use phxsql_store::memoria::{Consulta, Filtro, Operador, Ordem, TabelaMemoria};
use phxsql_store::table::{Table, Visao};

use crate::acesso::{Acesso, LogAcessos};
use crate::bidirecional::{self, EstadoOrigem, MapaDeToques, Toque};
use crate::blacklist::Blacklist;
use crate::config::{Config, Durabilidade, Papel};
use crate::dblink::{Definicao, Motor};
use crate::exportar::Formato;
use crate::http;
use crate::idiomas;
use crate::juncao::{Lado, Tipo as TipoJuncao, Uniao};
use crate::mensagens::Mensagens;
use crate::usuarios::{Atividade, Usuario};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;

use crate::pivot::{Agregador, Campo, Granularidade, Juncao};
use crate::valores::{
    bytes_para_hex, hex_para_bytes, json_para_chave, json_para_linha, json_para_valor_da_coluna,
    largura_do_tipo, linha_para_json,
};

// A divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`): cada
// dominio do `impl Servidor` mora num `servidor/servico_<dominio>_NN.rs`, e o
// `use` de cada um traz de volta o que o resto do servidor chama.
mod servico_admin_01;
mod servico_avisos_01;
mod servico_backup_01;
mod servico_bidirecional_01;
mod servico_cluster_01;
mod servico_composicao_01;
mod servico_config_01;
mod servico_consulta_01;
mod servico_dblink_01;
mod servico_diario_01;
mod servico_escrita_01;
mod servico_esquema_01;
mod servico_jobs_01;
mod servico_leitura_01;
mod servico_marca_01;
mod servico_mcp_01;
mod servico_nucleo_01;
mod servico_permissao_01;
mod servico_quorum_01;
mod servico_rede_01;
mod servico_replicacao_01;
mod servico_replicacao_02;
mod servico_sql_01;
mod servico_telemetria_01;
mod servico_transacao_01;
mod servico_web_01;
use servico_admin_01::*;
use servico_avisos_01::*;
use servico_backup_01::*;
use servico_bidirecional_01::*;
use servico_cluster_01::*;
use servico_composicao_01::*;
use servico_consulta_01::*;
use servico_escrita_01::*;
use servico_leitura_01::*;
use servico_marca_01::*;
use servico_nucleo_01::*;
use servico_quorum_01::*;
use servico_rede_01::*;
use servico_replicacao_01::*;
use servico_replicacao_02::*;
use servico_telemetria_01::*;
use servico_transacao_01::*;
use servico_web_01::*;
// Os modulos de teste, filhos daqui, leem por `use super::*` itens de dois
// dominios que o resto do servidor nao chama.
#[cfg(test)]
use servico_permissao_01::*;
#[cfg(test)]
use servico_sql_01::*;
// Os dois tipos publicos que sairam daqui continuam no caminho de sempre,
// `phxsql_server::servidor::*`, que os exemplos, os testes e o `main` usam.
pub use servico_mcp_01::ExecutorLocal;
pub use servico_rede_01::Remoto;

/// Uma linha de anuncio do arranque no erro padrao, numa escrita SO.
///
/// O `eprintln!` nao e uma escrita so: o `stderr` da `std` nao tem buffer, e
/// cada pedaco do formato -- o texto fixo, cada octeto do `SocketAddr` --
/// sai numa syscall propria (medido pelo pedido 581, `strace -e write`).
/// Quem le o arquivo no meio ve a linha pela metade, e a porta cortada em
/// `:43` de `:4321` ainda se le -- como OUTRA porta. O apoio `comum` dos
/// testes aprendeu a esperar o `\n`; os leitores de fora dele (os scripts
/// da `bancada/`, quem monitora o log) nao. Montar a linha inteira e
/// mandar de uma vez fecha pela origem (pedido 586).
///
/// Erro de escrita e ignorado: anuncio que nao saiu nao derruba o servidor
/// que ja subiu.
pub(crate) fn anunciar(linha: &str) {
    let mut inteira = String::with_capacity(linha.len() + 1);
    inteira.push_str(linha);
    inteira.push('\n');
    let _ = std::io::stderr().lock().write_all(inteira.as_bytes());
}

pub const VERSAO: &str = env!("CARGO_PKG_VERSION");

/// O emprestimo das MAES que a passada ja abriu mora no store desde o pedido
/// 563, junto da marca que o usa: a passada do COMMIT, a recuperacao e a
/// cascata do embutido perguntam pelo MESMO mapa.
pub(crate) use phxsql_store::marca::MaesAbertas;

/// Operacoes que alteram dados. Recusadas quando `somente_leitura` esta ligado.
pub(crate) const OPS_ESCRITA: &[&str] = &[
    "inserir",
    // As duas da sincronia: ligar cria tabela local, sincronizar grava nela.
    "dblink_ligar",
    "dblink_sincronizar",
    "atualizar",
    "excluir",
    "reindexar",
    "criar_database",
    "criar_schema",
    "criar_tabela",
    // Declarar e desdeclarar chave estrangeira regravam o bloco de esquema
    // no `.reg` -- catalogo, mas catalogo gravado em disco.
    "declarar_fk",
    "excluir_fk",
    // Redeclarar o indice de texto regrava o bloco de esquema e refaz -- ou
    // apaga -- o `.fts` (pedido 364).
    "redeclarar_indices_texto",
    // Acrescentar coluna reescreve o `.reg` inteiro. E a maior escrita de
    // estrutura que existe aqui.
    "acrescentar_coluna",
    // Criptografar/descriptografar (pedido 268) reescreve o `.reg` inteiro, a
    // mesma familia do `acrescentar_coluna`.
    "criptografar",
    "descriptografar",
    // Levar a tabela ao PSCH v10 reescreve o `.reg` inteiro UMA VEZ POR
    // COLUNA que falta -- e a mesma familia do `acrescentar_coluna`, porque e
    // ele que faz o trabalho. A VISTA PREVIA (sem `confirmar`) vem junto de
    // proposito, e nao por descuido: ela responde o custo da parada DESTE
    // servidor, e num somente-leitura a parada nunca acontece aqui. Numero de
    // parada que ninguem vai pagar e resposta do servidor errado.
    "migrar_esquema",
    "excluir_tabela",
    // As duas que gravam o `visoes.json` do database. Catalogo, mas catalogo
    // gravado em disco -- e num servidor somente-leitura ninguem cria visao.
    "criar_visao",
    "excluir_visao",
    "duplicar_tabela",
    "copiar_tabela",
    "renomear_tabela",
    "ajustar_sequencia",
    // A sequencia nomeada: criar e apagar mexem na pasta, e pedir o proximo
    // numero GRAVA -- e o que faz o numero valer.
    "criar_sequencia",
    "excluir_sequencia",
    "proximo_da_sequencia",
    "inserir_lote",
    // Reservar a tabela para carga e declarar intencao de gravar. Num servidor
    // somente-leitura ninguem vai carregar nada, e deixar reservar seria
    // deixar travar a tabela para uma escrita que nunca acontece.
    "bulkinsert",
    // Abrir e confirmar uma transacao, pelo mesmo argumento do `bulkinsert`:
    // abrir declara a intencao de gravar e reserva tabela, e confirmar grava.
    // `rollback` e `savepoint` NAO entram, e a ausencia e deliberada -- os
    // dois so mexem numa lista em RAM, e recusar o `rollback` num servidor
    // que virou somente-leitura no meio deixaria a transacao presa ate o
    // prazo, sem ninguem poder solta-la.
    "begin",
    "start_transaction",
    "begin_transaction",
    "commit",
    // Marcar e desmarcar mexem em dado gravado. Listar a lixeira e os
    // motivos, nao -- essas duas so leem, e continuam valendo no modo
    // somente leitura, que e justamente quando alguem esta investigando.
    "restaurar",
    // Restaurar um backup grava um database inteiro de uma vez -- e a maior
    // escrita que existe aqui. `backups`, que so lista o que ha na pasta, nao
    // entra: ler a pasta nao muda byte nenhum, e e justamente o que se quer
    // poder fazer num servidor somente-leitura antes de decidir.
    "restaurar_backup",
    // As duas que apagam sem volta um arquivo LOCAL do no. Sao escrita para
    // toda pergunta desta lista -- nao entram em transacao, esbarram na trava
    // de outra transacao, contam como escrita na telemetria e no catalogo --,
    // e so a do somente-leitura/replica responde diferente: ver `OPS_DO_NO`.
    "esvaziar_lixeira",
    "expurgar_trilha",
    // As tres que gravam na tabela de textos da tela. Num servidor somente
    // leitura semear nao pode gravar, e as outras duas apagam trabalho.
    "idiomas_carga",
    "idiomas_padrao",
    "idiomas_importar",
    // Gravam o cadastro de ligacoes, que e arquivo deste servidor.
    "dblink_salvar",
    "dblink_excluir",
    // `aplicar` NAO entra aqui, e a ausencia e deliberada. Uma replica roda em
    // `somente_leitura` justamente para a aplicacao nao escrever nela -- e a
    // unica escrita que ela deve aceitar e a que vem do source. Barrar
    // `aplicar` aqui tornaria impossivel replicar para uma replica protegida,
    // que e a unica replica que se sustenta. Quem pode chamar `aplicar` ja
    // passou pelo portao do `administrar`.
    "encerrar_sessao",
    // As tres do cadastro de usuarios, pela mesma razao do `config_gravar`
    // logo abaixo: um servidor declarado somente-leitura nao reescreve o
    // proprio config.json pela porta da rede -- nem para dizer quem entra
    // nele.
    "usuario_criar",
    "usuario_alterar",
    "usuario_excluir",
    // Grava o config.json, que e arquivo deste servidor -- mesma familia do
    // cadastro de DbLink e de jobs. Um servidor declarado somente-leitura nao
    // reescreve a propria configuracao pela porta web; tirar o
    // `somente_leitura` continua sendo edicao do arquivo, que e por onde ele
    // entrou.
    "config_gravar",
    // `ALTER … SET` desemboca no `config_gravar` (escopo de servidor) ou grava
    // a politica por banco: as duas reescrevem o `config.json`, e valem a
    // mesma regra.
    "diretiva_gravar",
    // Gravam o cadastro de jobs, que e arquivo deste servidor. `job_rodar` NAO
    // entra: ele confere o portao com a operacao DE DENTRO do job, entao um
    // job que grava ja e recusado por ela num servidor somente-leitura -- e um
    // job que so le continua rodando, que e o certo.
    "job_salvar",
    "job_excluir",
    "job_ligar",
    // Parar a porta de dados nao grava byte nenhum, mas interrompe o trabalho
    // de todo mundo -- pela mesma razao que `encerrar_sessao` esta aqui. Um
    // servidor declarado somente-leitura nao e um servidor sem dono.
    "servico_parar",
    "servico_subir",
    // `spare_promover` NAO entra aqui, e a ausencia e deliberada, como a do
    // `aplicar`: um spare roda com `somente_leitura` ligado, e e exatamente
    // nele que a promocao precisa funcionar. O portao dela e o `administrar`.
];

/// As escritas que mexem so no arquivo DESTE no, e nao no dado replicado --
/// pedidos 368 (C2) e 499.
///
/// # Por que uma segunda lista, e nao tirar do `OPS_ESCRITA`
///
/// O `OPS_ESCRITA` responde SEIS perguntas: o portao do somente-leitura e da
/// replica, a transacao (o que nao se empilha nao entra), a trava de outra
/// transacao, a telemetria, o `escreve` do catalogo e do OpenAPI. Tirar uma op
/// de la para acertar a primeira mudou as seis, e isso foi MEDIDO pelo papel C
/// (`docs/propostas/parecer-dba-faceis-c-2026-09-24.md`): `BEGIN;
/// esvaziar_lixeira; ROLLBACK` esvaziava e nao voltava, e o esvaziar passava
/// por cima da IX de outra transacao. Sao duas perguntas, e a lei dos iguais
/// vale para quem responde a MESMA -- entao duas listas, e esta so e lida por
/// [`grava_dado_replicado`].
///
/// # O que cada uma e
///
/// * `esvaziar_lixeira` -- a replica aplica a exclusao do source pelo
///   `excluir_de_vez` de sempre e guarda a linha inteira no `.trash` DELA; o
///   esvaziar do source nao vira evento e nao chega la. Barrada, a replica
///   somente-leitura recusava o proprio administrador, e a linha apagada no
///   source ficava no disco da replica para sempre (`tests/lixeira-da-replica.rs`).
/// * `expurgar_trilha` -- a trilha `.lgpd` e local: a replica escreve a dela
///   (o registro de ACESSO nasce de leitura) e nao a recebe do source.
///
/// As duas continuam pedindo `administrar` e `motivo`, e o rastro vai ao
/// `.reason` antes.
pub(crate) const OPS_DO_NO: &[&str] = &["esvaziar_lixeira", "expurgar_trilha"];

/// Esta op grava o dado que a replicacao carrega? E a pergunta do portao do
/// somente-leitura, da replica de leitura e do cluster -- e so dela. Ver
/// [`OPS_DO_NO`].
pub(crate) fn grava_dado_replicado(op: &str) -> bool {
    OPS_ESCRITA.contains(&op) && !OPS_DO_NO.contains(&op)
}

pub(crate) const OPS_NO_SPARE: &[&str] = &[
    // A sessao em si.
    "ping",
    "desafio",
    "login",
    "sair",
    "quem_sou",
    "catalogo",
    // Administracao e monitoramento.
    "config",
    "usuarios",
    "acessos",
    "ips",
    "bloqueios",
    "desbloquear",
    "sessoes",
    "processlist",
    "encerrar_sessao",
    "kill",
    "sistema",
    "painel",
    // A saude do disco do spare e monitoramento como o `painel`: e a reserva
    // que mais precisa dizer se o disco dela ainda aceita escrita.
    "saude_disco",
    "servico",
    "servico_parar",
    "servico_subir",
    "estatisticas",
    "estatisticas_uso",
    // Conferencia de integridade e de igualdade -- e como um administrador
    // confere a reserva sem abrir o dado linha a linha.
    "verificar",
    "checksum",
    "soma_de_verificacao",
    "diario",
    "backup",
    "conferir_backup",
    // Metadado: ver QUE bancos e tabelas existem nao e ler o dado.
    "bancos",
    "tabelas",
    "esquema",
    // A replicacao inteira, que e a razao de o spare existir.
    "posicao",
    "replicar",
    "retrato_da_replica",
    "aplicar",
    "replicacao_estado",
    "replicacao_testar",
    "replicacao_ligar",
    // Soltar o par que um conflito parou e administracao do laco, como o
    // religar: um spare que virasse multi amanha precisa da porta de saida.
    "replicacao_pular",
    "spare_promover",
];

/// O que uma REPLICA chama no source com a credencial de replicacao, e so isso.
///
/// E a lista que `replicacao.replicas_autorizadas` tranca. As tres primeiras
/// sao as da secao 6 do `docs/REPLICACAO.md`: `posicao` diz ate onde o diario
/// foi, `replicar` entrega os eventos com a linha dentro, e `aplicar` grava
/// com o rowid escolhido. Juntas, elas SAO o dado -- quem pode chamar as tres
/// leva a base inteira.
///
/// `cluster_pulso` e a quarta, desde 17/09/2026 (revisao SEC, A1): ela entra
/// com a MESMA credencial das outras tres -- a do cluster e a de `replicar`,
/// ver `usuarios.rs` -- e decide quem manda: epoca, posicao e papel de cada
/// no saem do pulso, e um pulso forjado destrona o master. A lista tranca
/// por operacao, e uma lista de operacoes envelhece como envelhece um campo
/// novo do portao: quando o portao passar a olhar um campo novo, procure
/// quem nao tem esse campo -- e quando entrar uma operacao com a credencial
/// de replicacao, procure se ela esta aqui. Consequencia para quem preenche a
/// lista num cluster: ela tem de trazer TODOS os outros nos, porque cada no
/// pulsa para cada outro.
///
/// `replicacao_estado`, `replicacao_testar` e `spare_promover` NAO entram, e a
/// ausencia e deliberada: sao operacoes de administracao, exigem
/// `administrar`, e quem administra nao e uma replica remota. Trancar essas
/// pela lista de replicas faria a lista significar duas coisas.
pub(crate) const OPS_DE_REPLICACAO: &[&str] = &[
    "posicao",
    "replicar",
    // O retrato com que a replica se refaz depois do expurgo (pedido 706):
    // e o dado inteiro, como o `replicar`, e a mesma lista o tranca.
    "retrato_da_replica",
    "aplicar",
    "cluster_pulso",
    "replicar_aguardar",
];

/// As operacoes de MANUTENCAO que criam, apagam ou renomeiam arquivos de
/// tabela SEM passar por `congelamento::congelar` -- e que por isso o
/// despachar recusa enquanto um backup esta na fase 1 (pedido 513, passo 2,
/// o `BLOCK_DDL` do MariaDB e o `LOCK INSTANCE FOR BACKUP` do MySQL).
///
/// Nao e correcao: a fase 2 compara a arvore inteira e acerta mesmo com
/// arquivo criado ou apagado no meio. E custo -- um `reindexar` reescreve o
/// `.ndx` inteiro e o joga na fase 2, que roda sem escritor. As reescritas
/// que congelam (`acrescentar_coluna`, `migrar_esquema`, a trilha LGPD) sao
/// recusadas pelo proprio `congelar`, no armazem: UMA pergunta, dois lados.
/// O que grava DADO (`inserir`, `alterar`, `excluir`, `commit`) nao esta
/// aqui, e e' esse o ganho do passo 2.
pub(crate) const OPS_DE_MANUTENCAO: &[&str] = &[
    "criar_database",
    "criar_schema",
    "criar_tabela",
    "excluir_tabela",
    "renomear_tabela",
    "duplicar_tabela",
    "copiar_tabela",
    "reindexar",
    "restaurar_backup",
];

/// Por onde esta sessao entrou.
///
/// Existe por causa de um campo que MENTIA: `encryption_exigida`, dentro de
/// `diretivas_da_conexao`, cuja propria documentacao diz «o que e verdade
/// DESTA conexao», publicava `cifra_fio.exigir` para qualquer um que
/// perguntasse -- inclusive para a conexao HTTP em claro que fazia a pergunta.
/// Campo que declara protecao maior que a prestada e a familia do
/// `recursos.cache_paginas`, que anunciava cache sem haver cache.
///
/// Saber a porta de entrada e o que permite responder a verdade, e por isso
/// mora na SESSAO e nao no pedido: e propriedade da CONEXAO, do mesmo jeito
/// que o `ip` e a `transcricao_do_fio`.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum Entrada {
    /// Nao ha fio nenhum: job agendado, rotina interna, replicacao aplicada,
    /// ponte MCP pelo cano do processo. E o padrao porque e o unico que nao
    /// promete nada -- quem tem fio diz qual.
    #[default]
    SemFio,
    /// A porta de DADOS, a unica onde mora o aperto de mao do fio.
    Dados,
    /// Uma das portas HTTP -- web, REST, explorador. Nunca ha tunel aqui: o
    /// navegador fala TLS ou fala claro, e o TLS e do proxy reverso
    /// (`docs/SEGURANCA.md` §7.1), que este servidor nao tem como conferir.
    Http,
}

/// Estado de uma conexao.
///
/// A senha e conferida com PBKDF2, que custa da ordem de 100 ms de proposito.
/// Fazer isso a cada pedido inviabilizaria o servidor, entao a autenticacao
/// acontece UMA VEZ por conexao e o resultado fica aqui.
#[derive(Default)]
struct Sessao {
    usuario: Option<Usuario>,
    /// A conexao desta sessao, do registro de ligacoes. Zero quando o pedido
    /// veio pela porta web, que nao tem conexao para amarrar nada.
    ///
    /// E a ela que a reserva de carga morre amarrada: sem um id de CONEXAO, a
    /// reserva so poderia ser identificada pelo login -- e aí duas janelas do
    /// mesmo usuario seriam o mesmo dono, o que e exatamente o contrario de
    /// exclusivo.
    ligacao: u64,
    /// Desafio em aberto: (usuario, nonce do servidor, quando expira).
    /// Vale uma vez so -- e consumido no login, dando certo ou errado.
    desafio: Option<(String, String, i64)>,
    /// De onde veio esta conexao.
    ///
    /// Existe para a trilha de LGPD, que precisa gravar o IP de quem alterou
    /// ou leu dado pessoal. Mora na SESSAO, e nao no pedido, porque e uma
    /// propriedade da conexao e nao de cada operacao -- e porque assim as
    /// quarenta operacoes que ja recebem `&Sessao` ganham o IP sem que
    /// nenhuma delas mude de assinatura.
    ///
    /// Vazio nos caminhos que nao tem conexao: replicacao, job agendado,
    /// rotina interna. Vazio ali e a verdade, e nao uma falta.
    ip: String,
    /// Em que geracao do cadastro esta ficha foi tirada.
    ///
    /// A autenticacao acontece uma vez por CONEXAO, e por isso a ficha aqui e
    /// uma copia: sem esta marca, quem foi excluido as 10h continuaria
    /// entrando ate as 18h, quando a conexao dele caisse. Ver
    /// `Servidor::refrescar_a_sessao`.
    geracao_do_cadastro: u64,
    /// A transcricao do aperto, quando esta conexao passou pelo tunel.
    ///
    /// Mora na SESSAO pelo mesmo motivo do `ip`: e propriedade da CONEXAO, e
    /// nao de cada pedido, entao o `op_login` a le sem que a assinatura de
    /// `despachar` mude. `None` = conexao em claro (o padrao, e a porta web,
    /// que fala HTTP e nao tem tunel). E o que o `login` amarra a credencial
    /// quando o cliente pede `amarrar_canal`. Ver `docs/CIFRA-DO-FIO.md` §10.
    transcricao_do_fio: Option<[u8; 32]>,
    /// Por onde esta conexao entrou -- ver [`Entrada`].
    entrada: Entrada,
    /// O `ip` acima e o do PROXY declarado, e nao o de quem pediu (pedido
    /// 284).
    ///
    /// Numa porta HTTP com `atras_de_proxy`, o par do soquete e o proxy
    /// reverso, e todo cliente de fora chega com o mesmo endereco. Mora na
    /// SESSAO pelo mesmo motivo do `ip`: e propriedade da conexao, e o portao
    /// que compara o IP com uma lista precisa saber se o IP diz quem pediu.
    ip_do_proxy: bool,
    /// O fio desta conexao e TLS nativo (pedido 572): `FioWeb::Tls` nas
    /// portas HTTP, a `Escrita::Tls` na de dados. Mora na SESSAO pelo motivo
    /// do `ip`, e e o que o [`Servidor::fio_cifrado`] le -- antes dele, a
    /// conexao TLS da porta de dados contava como «em claro» para o dado
    /// pessoal, e a da web so contava cifrada com `exigir` ligado.
    fio_tls: bool,
}

impl Sessao {
    fn login(&self) -> &str {
        self.usuario
            .as_ref()
            .map(|u| u.login.as_str())
            .unwrap_or("")
    }

    /// Id gravado no `.log` da tabela como autor da operacao.
    /// Zero quando a conexao veio pelo token de servico, sem login.
    fn id(&self) -> u32 {
        self.usuario.as_ref().map(|u| u.id).unwrap_or(0)
    }
}

pub struct Servidor {
    config: Config,
    /// Trava unica de dados. Ver a nota de concorrencia no topo do modulo.
    ///
    /// # Por que um `RwLock`, e por que ele NAO guarda a `Instancia`
    ///
    /// `RwLock<Instancia>` compila de primeira e esta errado -- todo metodo da
    /// `Instancia` e `&self`, que e o que um guard de LEITURA entrega, entao
    /// dois escritores abririam dois `Table` sobre os mesmos arquivos sem um
    /// erro do compilador. O marcador `!Sync` da `Instancia` existe para
    /// transformar esse engano silencioso em erro de compilacao, e ele
    /// continua no lugar.
    ///
    /// O que esta aqui e a [`Raiz`], que separa as duas fichas pelo tipo de
    /// emprestimo: `&Raiz` (N leitores ao mesmo tempo) so alcanca tabela de
    /// LEITURA; `&mut Raiz` (um de cada vez) alcanca a `Instancia` inteira.
    dados: RwLock<Raiz>,
    /// O portao do retrato -- pedido 513. Durante a copia do backup a
    /// escrita espera AQUI, antes da fila do `RwLock`, e nao dentro dela:
    /// escritor na fila faz o leitor novo esperar tambem, e ai a ficha
    /// compartilhada do backup nao comprava nada. Ver `crate::retrato`.
    retrato: crate::retrato::PortaoDoRetrato,
    /// Quantos panicos desenrolaram com a trava de dados na mao, e quantos
    /// deles o reparo TERMINOU -- pedido 451.
    ///
    /// # Por que dois contadores, e nao o veneno do `RwLock`
    ///
    /// O veneno nao sai, e de proposito (pedido 653). A razao velha era a
    /// versao -- o `clear_poison` e de 1.77 e a casa prometia 1.75 --, e ela
    /// acabou com o `rust-version` em 1.89. A que fica: o `RwLock` se
    /// envenena quando a GUARDA cai, e quem a segura no desenrolar e o
    /// proprio `TravaMedida::drop`, que repara e so DEPOIS solta -- o reparo
    /// nao tem como limpar um veneno que ainda nao foi posto. Limpar na
    /// tomada seguinte custaria uma escrita na trava para poupar as duas
    /// leituras atomicas abaixo, que a tomada envenenada ja paga, e nao
    /// mudaria decisao nenhuma: quem decide e ESTE par, e nao o veneno. A
    /// prova de que a trava volta a atender com o veneno permanente, tomada
    /// apos tomada, e `o_veneno_permanente_continua_recuperando_tomada_apos_tomada`.
    /// Entao a trava fica envenenada para sempre depois do
    /// primeiro panico, e o que decide se ela volta a atender e ESTE par: com
    /// os dois iguais, todo panico que a sujou ja passou pelo reparo do
    /// `TravaMedida::drop`, e o estado do disco e o que o arranque deixaria.
    /// Com eles diferentes, algum panico a sujou sem o reparo terminar, e ela
    /// falha FECHADA como antes -- o `SP000010` que nomeia a trava.
    ///
    /// O panico conta ANTES do reparo, e o reparo conta DEPOIS de terminar:
    /// quem ler os dois no meio (ninguem le, porque a trava ainda esta na mao
    /// de quem repara) veria «falta reparo», que e o lado seguro.
    panicos_na_trava: AtomicU64,
    reparos_da_trava: AtomicU64,
    /// Quando o gravado vai de fato para o disco.
    janela: Janela,
    /// Tabelas escritas desde o ultimo `fsync`, como "database/tabela".
    ///
    /// Existe porque a tabela e aberta e fechada a cada operacao: quando a
    /// janela fecha, quem esta aberto e so a tabela da operacao corrente, e as
    /// outras tocadas na janela ficariam sem sincronizar. Este conjunto e a
    /// lista do que ainda deve ao disco.
    sujas: Mutex<std::collections::HashSet<String>>,
    /// Serializa os `proximo` das sequencias NOMEADAS (pedido 229). Nao e a
    /// trava global de dados de proposito: o `proximo` faz um `fdatasync`
    /// por numero, e isso nao pode acontecer com a trava global na mao (a
    /// catraca `alcancam-fsync-3`). A trava global entra so para ACHAR o
    /// arquivo; o disco se espera aqui.
    trava_das_sequencias: Mutex<()>,
    log: Mutex<LogAcessos>,
    lista_negra: Mutex<Blacklist>,
    /// Sessoes do navegador. Vazio enquanto a interface web estiver desligada.
    sessoes: Mutex<http::Sessoes>,
    /// Tabelas residentes em RAM, por "database/tabela". Nada entra aqui
    /// sozinho: so o que alguem pediu para carregar.
    residentes: Mutex<HashMap<String, TabelaMemoria>>,
    /// Conexoes abertas para outros PhxSql, uma por sessao do navegador.
    ///
    /// Ficam abertas de proposito: o protocolo da porta 5000 autentica uma vez
    /// por CONEXAO, entao manter o soquete e o que faz o PBKDF2 do servidor
    /// remoto rodar uma vez por login e nao a cada clique.
    remotos: Mutex<HashMap<String, Arc<Mutex<Remoto>>>>,
    /// As conexoes vivas, para o operador ver quem esta falando e poder
    /// derrubar quem travou.
    ligacoes: Mutex<crate::ligacoes::Ligacoes>,
    /// Quando o servidor subiu, para o `ping` poder dizer ha quanto tempo.
    ///
    /// Um servidor que reiniciou sozinho de madrugada parece igual a um que
    /// nunca caiu -- ate alguem olhar o tempo no ar e ver duas horas.
    desde_ms: i64,
    /// Amostra anterior da maquina, para as taxas do painel.
    ///
    /// Guardar aqui, e nao na tela, e o que permite dizer "CPU em 40%": o
    /// `/proc` so traz contadores desde o arranque, e taxa exige duas
    /// amostras. Uma unica trava para todos os navegadores tambem evita cada
    /// aba abrir a propria serie e nenhuma delas fechar conta.
    monitor: Mutex<crate::sistema::Monitor>,
    /// Ultimo aviso mandado por caminho, para nao repetir enquanto o disco
    /// continua cheio.
    avisados: Mutex<HashMap<String, i64>>,
    /// A saude do disco onde o banco grava (pedido 249): a sonda canario, o
    /// contador de erros de E/S e o silencio por tipo. O vigia de ESPACO
    /// acima pergunta «quanto falta»; esta pergunta «o disco ainda aceita
    /// escrita?» -- e a segunda nao espera relogio nenhum para avisar.
    saude: Arc<crate::saude_do_disco::SaudeDoDisco>,
    /// A chave do HMAC do sal falso do `desafio` (pedido 528): segredo que so
    /// o servidor tem, e nao o token que todo cliente tem. Nunca sai em
    /// resposta, `Debug`, profiler nem `config`.
    segredo_do_desafio: [u8; 32],
    /// O que esta chegando pela porta, quando alguem liga para olhar.
    /// Tabelas reservadas para carga (`BULKINSERT`).
    cargas: Mutex<crate::carga::Cargas>,
    /// As marcas `.tx` de commits ja aplicados cuja janela de durabilidade
    /// ainda nao fechou. **E o group commit deste motor.**
    ///
    /// # Por que a marca pode esperar, e o que ela garante enquanto espera
    ///
    /// Porque quem decide se a transacao aconteceu e a MARCA, e nao o `fsync`
    /// da tabela: a marca ja esta sincronizada quando a passada comeca, e uma
    /// queda depois dela faz a recuperacao COMPLETAR o commit. Entao adiar o
    /// `fsync` da tabela nao adia a decisao -- adia so o momento em que o dado
    /// alcanca o disco, e a marca e o bilhete que o traz de volta.
    ///
    /// A ordem e a peca: a marca so e apagada DEPOIS de a tabela sincronizar.
    /// Apagar antes abriria a janela em que o dado nao esta no disco e nao ha
    /// bilhete nenhum para recupera-lo -- que e a mesma janela sem conserto da
    /// lixeira, pelo outro lado.
    marcas_pendentes: Mutex<Vec<PathBuf>>,
    /// Quem segura qual tabela e qual linha. Ver `crate::travas`.
    ///
    /// Trava PROPRIA, e nunca tomada com a de dados na mao por mais de uma
    /// consulta: a espera por uma trava de transacao acontece FORA da trava de
    /// dados, senao uma transacao esperando outra seguraria o servidor
    /// inteiro -- que e a doenca que o `alcancar_tabela_bidi` ja mediu em
    /// 29.456 ms.
    travas: crate::pulso::TravaDaGuarda<crate::travas::Travas>,
    /// As transacoes abertas, por conexao. Ver `crate::transacao`.
    transacoes: crate::pulso::TravaDaGuarda<crate::transacao::Transacoes>,
    /// Quantas transacoes estao abertas AGORA.
    ///
    /// # O portao que vem ANTES do trabalho
    ///
    /// E a licao que o Profiler cobrou, aplicada aqui de proposito: um
    /// `AtomicUsize` lido com `Relaxed` custa uma instrucao e nao serializa
    /// ninguem. Zero transacoes abertas -- que e o servidor inteiro hoje -- e
    /// NENHUMA estrutura de transacao e consultada: nenhum `Mutex` e tomado,
    /// nenhuma `String` e montada, nenhum campo do pedido e lido de novo.
    ///
    /// A janela de divergencia e de um pedido, e ela e inofensiva: quem abre
    /// uma transacao ja incrementou este contador ANTES de responder, entao o
    /// proximo pedido daquela conexao ja a enxerga.
    transacoes_abertas: AtomicUsize,
    /// Onde a ultima leitura do diario de cada tabela parou, por
    /// `database/tabela`.
    ///
    /// # Por que aqui, e nao na tabela
    ///
    /// A tabela e aberta e fechada a cada pedido, entao a marca morreria entre
    /// um `replicar` e o seguinte -- que sao exatamente os dois pedidos em que
    /// ela vale. Sem ela, servir «500 eventos a partir de P» caminha pelos P
    /// anteriores lendo o cabecalho de cada um, e alcancar N eventos custa
    /// N^2/2 leituras.
    ///
    /// Medido em `--example custo-do-desde`, num diario de 100.000 eventos:
    /// ler 500 a partir de 0 custa 1,11 us por evento; a partir de 90.000,
    /// 72,65. Alcancar os 100.000 de 500 em 500 gastava 4,07 s so aqui.
    ///
    /// E so uma DICA: perde-la custa uma varredura, e uma errada faz o CRC do
    /// evento recusar. Por isso ela nao vai a disco.
    ///
    /// # Por que uma LISTA, e nao uma marca por tabela
    ///
    /// Um source serve varias replicas, e elas nao estao na mesma posicao --
    /// uma que ficou fora do ar volta atras das outras. Com uma marca so, a
    /// que estivesse mais adiantada a moveria para frente e as outras nunca a
    /// aproveitariam: a marca so serve para uma posicao DEPOIS dela. Guardar
    /// algumas e escolher a maior que ainda cabe atende todas.
    marcas_do_diario: Mutex<HashMap<String, Vec<phxsql_store::log::MarcaDoDiario>>>,
    /// A ultima conferencia de continuidade de cada tabela replicada:
    /// `"database/tabela"` -> (posicao local conferida, o diario do source
    /// continuava o daqui?). Ver `alcancar_tabela`. Uma tabela ociosa e
    /// conferida UMA vez por posicao -- e nao a cada rodada --, e uma
    /// recusada fica recusada ate a posicao local mudar.
    continuidade_da_replica: Mutex<HashMap<String, (u64, bool)>>,
    profiler: Mutex<crate::profiler::Profiler>,
    /// Espelho de `profiler.ligado`, para o caminho quente nao tomar a trava.
    ///
    /// # Por que existe
    ///
    /// Observacao que nao esta ligada nao pode custar nada. Sem este espelho,
    /// TODO pedido pagava, antes de a conferencia acontecer: dois
    /// `Json::analisar` do corpo inteiro -- um para achar database/tabela,
    /// outro para o nome da operacao --, tres `String` alocadas, e um mutex.
    /// Num `inserir_lote` de cinco mil linhas isso e analisar meio megabyte de
    /// JSON duas vezes, para no fim `chegou` olhar `ligado` e devolver `None`.
    /// Medido: 7% da carga pela rede.
    ///
    /// Um `AtomicBool` lido com `Relaxed` custa uma instrucao e nao serializa
    /// ninguem. A trava so e tomada quando ha o que registrar.
    ///
    /// A janela de divergencia e de um pedido: quem liga o profiler pode nao
    /// ver o pedido que ja estava em voo. Ligar a observacao no meio de um
    /// pedido nao promete pegar aquele pedido -- promete pegar os proximos.
    profiler_ligado: AtomicBool,
    /// Gatilhos e procedimentos, por database, com os corpos ja compilados.
    rotinas: Mutex<crate::rotinas::Rotinas>,
    /// Espelho de "existe algum gatilho?", para o caminho de escrita SEM
    /// gatilho custar um load atomico e nada mais — nem trava, nem String.
    ///
    /// E a mesma decisao do `profiler_ligado`, tomada ANTES de doer: o portao
    /// que decide se ha trabalho vem antes de qualquer trabalho.
    ha_gatilhos: AtomicBool,
    /// As visoes (`CREATE VIEW`), por database, guardadas como TEXTO.
    visoes: Mutex<crate::visoes::Visoes>,
    /// A vez de gravar o cadastro por database (`gatilhos.json`,
    /// `procedimentos.json`, `visoes.json`) -- pedido 595. Separada das
    /// travas dos registros para o `fsync` nao segurar quem so le; ver
    /// `gravar_rotinas`.
    cadastro_no_disco: Mutex<()>,
    /// Espelho de "existe alguma visao?", pelo mesmo motivo do `ha_gatilhos`:
    /// a op `sql` pergunta isto ANTES de olhar o nome do `FROM`, e num
    /// servidor sem visao nenhuma -- que e o de hoje -- ela paga um load
    /// atomico e nada mais. O portao que decide se ha trabalho vem antes do
    /// trabalho.
    ha_visoes: AtomicBool,
    /// Ligacoes para bancos de fora.
    dblink: Mutex<crate::dblink::Registro>,
    /// Jobs de execucao: cadastro e a hora da ultima corrida de cada um.
    jobs: Mutex<crate::jobs::Registro>,
    /// O relogio dos jobs subiu neste processo?
    relogio_de_jobs: AtomicBool,
    /// Os jobs em execucao NESTE instante -- para a tela dizer "rodando" e o
    /// vigia nao confundir corrida longa com job parado.
    ///
    /// Trava propria, e nunca tomada com a de `jobs` na mao: quem precisa das
    /// duas tira a foto daqui ANTES de trancar o cadastro.
    jobs_rodando: Mutex<Vec<String>>,
    /// As corridas que o arranque fechou como FALHOU (pedido 502), esperando
    /// o `subir_jobs` para irem ao aviso por e-mail como qualquer falha --
    /// C2 do parecer do DBA. Esvaziada uma vez.
    interrompidas_a_avisar: Mutex<Vec<crate::jobs::Corrida>>,
    /// Quando cada aviso de job saiu por e-mail, por chave `falha:nome` /
    /// `parado:nome` -- o silencio entre avisos repetidos, como o do disco.
    avisos_de_jobs: Mutex<HashMap<String, i64>>,
    /// Um expurgo da trilha por vez no processo (pedido 368).
    ///
    /// O motor do expurgo solta a trava global entre planejar e apagar, para o
    /// `fsync` do rastro nao segurar o servidor inteiro. Sem esta trava, o
    /// relogio da retencao e o administrador poderiam planejar o MESMO volume
    /// ao mesmo tempo -- e o rastro sairia duas vezes para um apagamento so.
    /// Nunca e tomada com a trava de dados na mao: o motor a pega ANTES.
    expurgo_da_trilha: Mutex<()>,
    /// Um expurgo do diario por vez no processo (pedido 706), pelo mesmo
    /// motivo do da trilha: o motor solta a trava entre planejar e apagar.
    expurgo_do_diario: Mutex<()>,
    /// O que cada consumidor declarado em `diario.consumidores` confirmou ter
    /// do diario de cada tabela, por `consumidor` e chave `db/tabela`: o
    /// maior `desde` que ele pediu (pedido 706). So na memoria, e de
    /// proposito: perdido no reinicio, o expurgo SEGURA ate o consumidor
    /// pedir de novo -- a falha cai do lado de guardar, nunca de soltar.
    confirmados_do_diario: Mutex<HashMap<(String, String), u64>>,
    /// Os retratos que esta origem serve a replicas que se refazem (pedido
    /// 706): um por CONEXAO, amarrado a ela, ao login e ao database, ate o
    /// teto `TETO_DE_RETRATOS` -- pedidos 727 e 729. Era um slot global, e a
    /// segunda replica atrasada apagava o retrato da primeira.
    retrato_servido: Mutex<Vec<servico_diario_01::RetratoServido>>,
    /// Quando o aviso de VIOLACAO GRAVE de cada IP saiu por e-mail, por chave
    /// `grave:<ip>` -- o mesmo silencio dos jobs e do disco.
    avisos_de_seguranca: Mutex<HashMap<String, i64>>,
    /// A porta de dados esta aceitando conexao agora?
    ///
    /// Parada, o processo continua vivo e a interface web continua no ar --
    /// e e por ela que a porta volta. Um botao que derrubasse o PROCESSO nao
    /// teria como se desfazer: nao sobraria ninguem para atender o "subir".
    porta_no_ar: AtomicBool,
    /// Sinalizador que o laco de aceitacao le depois de cada `accept`.
    parar_de_aceitar: AtomicBool,
    /// O ouvinte ja preso no endereco novo, esperando o laco soltar o velho.
    ///
    /// A ordem importa e e a garantia contra o tiro no pe: o endereco novo e
    /// PRESO antes de o antigo ser solto. Porta ocupada ou endereco invalido
    /// falham enquanto o servico continua no ar, e nada muda.
    proximo_ouvinte: Mutex<Option<TcpListener>>,
    /// Onde a porta de dados escuta agora, que nem sempre e o `bind`.
    endereco_dos_dados: Mutex<Option<SocketAddr>>,
    /// Onde as tres portas HTTP REALMENTE ligaram -- pedido 401.
    ///
    /// Sem isto, um teste que pede `bind: "127.0.0.1:0"` (a porta que o
    /// sistema escolher) nao tinha como saber QUAL porta o sistema deu: a
    /// unica saida era ESCOLHER um numero antes (reservar, soltar, escrever
    /// no config) e torcer para ninguem mais pegar o mesmo numero entre o
    /// soltar e o `bind` de verdade -- a corrida que o pedido 401 fecha. As
    /// tres nascem uma vez so, no arranque (`subir_web`/`subir_rest`/
    /// `subir_swagger`), e nunca trocam depois -- ao contrario da porta de
    /// dados, que tem `servico_subir` para religar noutro endereco.
    endereco_web: Mutex<Option<SocketAddr>>,
    endereco_rest: Mutex<Option<SocketAddr>>,
    endereco_swagger: Mutex<Option<SocketAddr>>,
    /// As vagas da porta de dados: uma [`Permissao`] por conexao viva, teto
    /// `conexoes_max`. Era um `AtomicUsize` com `fetch_add` ao aceitar e
    /// `fetch_sub` no fim do fecho da thread -- e um panico dentro do
    /// `atender` pulava o `fetch_sub`. A permissao morre no `Drop`, que roda
    /// no desenrolar do panico, e a vaga volta. Pedido 248.
    permissoes_de_dados: Semaforo,
    /// As vagas das TRES portas HTTP juntas (interface, REST, explorador),
    /// teto `recursos.conexoes_web_max`. Antes nao havia teto nenhum: uma
    /// enxurrada de pedidos virava uma enxurrada de threads.
    permissoes_http: Semaforo,
    /// Ate quando (em ms de relogio) a fila HTTP esta declarada CHEIA.
    ///
    /// Quando um pedido esperou `fila_web_ms` inteiro e nao achou vaga, quem
    /// chegar logo depois esperaria o mesmo e receberia a mesma resposta --
    /// entao recebe na hora. Sem isto o aceitador, que e uma thread so,
    /// entregaria um 503 a cada `fila_web_ms` numa saturacao longa, e a fila
    /// do sistema operacional estouraria por tras dele. Zera na primeira
    /// vaga que aparece.
    http_cheia_ate_ms: AtomicU64,
    /// Quantos `atender` seguintes DESTE servidor devem entrar em panico.
    /// So existe nos testes -- e por servidor, e nao global, porque os
    /// testes do binario rodam em paralelo e um contador global faria a
    /// conexao de um teste vizinho cair no panico deste.
    #[cfg(test)]
    panicos_de_teste: AtomicUsize,
    /// Quantas conexoes o laco de aceitacao da porta de dados JA ACEITOU, e
    /// quantas ele recusou por falta de vaga. So existe nos testes, e existe
    /// para separar duas causas que o «nem todos os panicos aconteceram»
    /// confundia: «a conexao entrou e nao panicou» e «a conexao nunca foi
    /// aceita» sao defeitos diferentes, e um contador so nao as distingue.
    #[cfg(test)]
    aceitas_de_teste: AtomicUsize,
    #[cfg(test)]
    sem_vaga_de_teste: AtomicUsize,
    /// So nos testes: a passada de commit QUEBRA antes da escrita de numero N
    /// (1 = a primeira), com o erro que o congelamento da. Zero desliga.
    ///
    /// Existe porque o conserto do pedido 426 fecha por fora o caminho natural
    /// ate o braco de erro DEPOIS da marca -- a reescrita cede a transacao, e o
    /// commit recusa antes da marca --, e o braco continua existindo para a
    /// quebra que ninguem previu (E/S, disco cheio). Sem uma quebra de
    /// verdade ali dentro, o braco que completa a transacao nao teria prova.
    #[cfg(test)]
    passada_quebra_na_escrita: AtomicUsize,
    /// Com a quebra acima: a tabela da escrita quebrada fica CONGELADA ate o
    /// teste soltar -- a quebra que a recuperacao da hora tambem nao vence.
    #[cfg(test)]
    passada_quebra_congelando: Mutex<Option<phxsql_store::congelamento::Congelada>>,
    #[cfg(test)]
    passada_quebra_congela: std::sync::atomic::AtomicBool,
    /// So nos testes: roda UMA vez na janela sem a trava global do
    /// `declarar_fk` e do `marcar_lgpd` -- depois da varredura e da FASE A,
    /// antes de retomar a trava, com as tabelas congeladas (pedido 422). E o
    /// que torna a prova deterministica: o teste grava na filha, apaga o pai,
    /// usa a vizinha e olha os `*.novo` exatamente dentro da janela, sem
    /// depender de corrida nenhuma.
    #[cfg(test)]
    na_janela_sem_trava: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// So nos testes: roda UMA vez no `criar_tabela`, entre soltar a trava e
    /// levar a tabela ao disco -- a janela do pedido 605, onde o terceiro
    /// entra sem depender de corrida.
    #[cfg(test)]
    na_janela_da_criacao: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    /// So nos testes: a PROXIMA operacao com este nome arma, na thread que a
    /// atende, o panico de teste do motor (`ndx::panico_de_teste`) -- pedido
    /// 451. O panico e o do motor, no meio de uma escrita de verdade e com a
    /// trava de dados na mao; o que este campo acrescenta e so ESCOLHER a
    /// thread, porque a arma do motor e por thread e a conexao roda na dela.
    ///
    /// Por nome de operacao, e nao «a proxima tomada da trava»: o relogio de
    /// fundo tambem toma a trava, e armar a thread dele deixaria a arma presa
    /// num fio que nunca insere -- o teste esperaria um panico que nao vem.
    #[cfg(test)]
    panico_de_teste_na_op: Mutex<Option<(String, PanicoDeTeste)>>,
    /// So nos testes: o reparo da trava falha de proposito, para a prova do
    /// piso (H5) -- o processo tem de cair em vez de servir.
    #[cfg(test)]
    reparo_falha_de_teste: AtomicBool,
    /// So nos testes: quantos panicos a thread de pulso ainda da, um por volta
    /// do laco (`u32::MAX` = sempre) -- pedido 452.
    #[cfg(test)]
    panicos_no_pulso_de_teste: std::sync::atomic::AtomicU32,
    /// So nos testes: quantas vezes o gancho do pulso ja entrou em panico.
    #[cfg(test)]
    panicos_no_pulso_dados: std::sync::atomic::AtomicU32,
    /// So nos testes: o relogio de jobs entra em panico na proxima volta,
    /// FORA da corrida (o irmao do pedido 452). Dispara uma vez.
    #[cfg(test)]
    panico_no_relogio_de_jobs_de_teste: AtomicBool,
    /// So nos testes: o amostrador entra em panico na proxima volta (irmao do
    /// pedido 452). Dispara uma vez.
    #[cfg(test)]
    panico_no_amostrador_de_teste: AtomicBool,
    /// So nos testes: o reparo da trava entra em panico -- o panico duplo, que
    /// o Rust transforma em `abort` (pedido 451, M3).
    #[cfg(test)]
    reparo_panica_de_teste: AtomicBool,
    /// So nos testes: a releitura da marca em voo, no reparo, falha com erro
    /// de E/S (pedido 451, M4). Injetado no `ler_marca`, e nao provocado no
    /// sistema: o `EMFILE` de verdade exigiria baixar o limite de descritores
    /// do processo inteiro, e o `chmod 000` nao impede a leitura de root. E o
    /// arquivo tem de continuar la, inteiro -- e o apagamento dele que a
    /// prova mede.
    #[cfg(test)]
    marca_ilegivel_de_teste: AtomicBool,
    /// So nos testes: o fecho da janela entra em panico ao chegar na tabela
    /// `.0` (vazia = a primeira), e so na thread cujo nome comeca com `.1`
    /// (vazio = qualquer) -- pedido 451, A1 e A2. Dispara uma vez.
    #[cfg(test)]
    panico_no_fecho_de_teste: Mutex<Option<(String, String)>>,
    /// So nos testes: o fecho da janela da esta tabela como NAO sincronizada
    /// enquanto o campo estiver preenchido -- o erro de E/S que prende a marca.
    #[cfg(test)]
    fecho_falha_de_teste: Mutex<Option<String>>,
    /// Desliga a pre-conferencia do COMMIT (pedido 448): e o DEFEITO REPOSTO
    /// das provas dele, e e o que mantem o cinto da passada exercitado -- sem
    /// isto, o braco «erro do dado com parte gravada» ficaria sem prova.
    #[cfg(test)]
    pre_conferencia_desligada: std::sync::atomic::AtomicBool,
    /// Desliga a pergunta pela trava de linha da escrita solta (pedido 561):
    /// e o escritor que nao pergunta, e mantem exercitado o cinto do
    /// `refazer_o_elo` (537), que o 561 deixou sem caminho pelo protocolo.
    #[cfg(test)]
    solto_sem_trava_de_linha: std::sync::atomic::AtomicBool,
    /// O estado vivo do cluster -- `None` quando o `config.json` nao traz o
    /// bloco `cluster`, e ai NADA disto existe: nenhuma thread, nenhum portao.
    cluster: Option<Arc<crate::cluster::EstadoCluster>>,
    /// O cubo do quorum de escrita (pedido 207). Existe com o bloco
    /// `cluster`, e com `quorum_minimo` zero nao faz nada: o portao da
    /// `travar_dados` le o minimo antes de ligar qualquer anotacao.
    quorum: Option<Arc<crate::quorum::Cubo>>,
    /// As mensagens que o servidor devolve, resolvidas pela tabela
    /// `phxsys.mensagens` quando ela existe. Ver `mensagens.rs`.
    mensagens: Mensagens,
    /// O papel VIVO deste processo -- nasce igual ao do `config.json` e so
    /// muda pela promocao (`spare_promover`). Atomico porque o portao de
    /// pedido le a cada operacao, e um mutex ali serializaria todo mundo por
    /// um valor que muda uma vez na vida.
    papel_vivo: AtomicU8,
    /// O `somente_leitura` VIVO. A promocao de um spare precisa abrir a
    /// escrita sem reiniciar o processo -- e o config continua dizendo o que
    /// esta no arquivo, que e outra pergunta.
    somente_leitura_vivo: AtomicBool,
    /// As proibicoes POR BANCO, vivas: `(banco, comando)`.
    ///
    /// Vivas porque `ALTER DATABASE … SET comandos_proibidos` e um APERTO, e
    /// aperto que so valesse no proximo arranque deixaria aberta justamente a
    /// janela em que alguem esta fechando a porta.
    proibidos_por_base: Mutex<Vec<(String, String)>>,
    /// Ha alguma? **O portao que decide vem ANTES do trabalho.**
    ///
    /// Sem esta bandeira, todo pedido de todo cliente pagaria um `lock` no
    /// caminho quente do `despachar` por uma lista que, em quase todo
    /// servidor, esta vazia. E a mesma licao que o Profiler desligado cobrou a
    /// 7% da carga, e o mesmo remedio do `ha_gatilhos`.
    ha_proibidos_por_base: AtomicBool,
    /// O diario administrativo: quem mudou qual diretiva, quando e por que.
    diario: crate::diretivas::Diario,
    /// O que cada laco de replica conta, por nome de origem, para a operacao
    /// `replicacao_estado` -- posicao, ultimo erro, recusas.
    estado_replicacao: Mutex<HashMap<String, EstadoOrigem>>,
    /// De que origem vem cada nome de database. Ver [`DonoDoDatabase`].
    ///
    /// Vazio e intocado em todo servidor que puxa de uma origem so, que e o
    /// caso comum: quem o enche e [`Servidor::subir_replicacao`], e so com
    /// mais de uma origem em paralelo.
    dono_do_database: Mutex<HashMap<String, DonoDoDatabase>>,
    /// Ha mais de uma origem puxando em PARALELO? **O portao que decide vem
    /// antes do trabalho.**
    ///
    /// Sem esta bandeira, toda rodada de todo par 1<->1 pagaria um `lock` e
    /// uma varredura por um mapa que naquele servidor nunca tem nada -- a
    /// mesma licao que o Profiler desligado cobrou a 7% da carga. Ela nasce
    /// falsa e so vira verdadeira onde o paralelismo existe: uma thread por
    /// origem, duas ou mais. Com `cluster`, `subir_replicacao` volta antes e
    /// a bandeira FICA falsa de proposito -- ali quem puxa e um laco so, do
    /// master corrente, e o nome da origem MUDA a cada eleicao
    /// (`cluster:<id>`); um dono guardado por nome recusaria ao master novo o
    /// database do master velho, que e a promocao inteira parando.
    ha_varias_origens: AtomicBool,
    /// O ultimo toque por chave, por "database/tabela", para o conflito do
    /// bidirecional. Reconstruido do proprio diario; perder custa varredura.
    toques_bidi: Mutex<HashMap<String, MapaDeToques>>,
    /// Pedido 300 §2.7: `(orfas, sem_conferir)` por "database/tabela" -- as
    /// linhas que a replicacao (fiel ou bidirecional) gravou aqui sem a mae.
    /// So sobe, e e estado de processo, como os contadores irmaos do
    /// `replicacao_estado`. Ver `phxsql_store::table::ContagemDeOrfas`.
    orfas_na_replica: Mutex<HashMap<String, (u64, u64)>>,
    /// Pedido 300 (4): quantos pedidos de escrita LOCAL esta replica fiel
    /// aceitou, por "database/tabela" (a chave do diario). A escrita local
    /// toma o lugar do evento seguinte do source no diario daqui; a
    /// conferencia de continuidade para a tabela, e este numero e o que a
    /// deixa dizer POR QUE -- em vez de culpar o source.
    escritas_locais_na_replica: Mutex<HashMap<String, u64>>,
    /// Posicao consumida por "origem|database/tabela" no modo bidirecional.
    /// Persistida em `replicacao-posicoes.json` ao lado dos dados.
    posicoes_bidi: Mutex<HashMap<String, u64>>,
    /// Pedido 329: o dono de cada numero de origem que este no ja viu -- ele
    /// mesmo, as origens que puxa e os que puxam dele. Persistido em
    /// `replicacao-numeros.json`; ver `bidirecional::conferir_numero`.
    numeros_bidi: Mutex<std::result::Result<bidirecional::Numeros, String>>,
    /// Pedido 424: quantas tabelas de ledger com coluna marcada esta replica
    /// CRIOU a partir do esquema de um source. A replica nao recusa -- a
    /// petrea «guarda nova entra pedida» ganha da regua dos motores --, entao
    /// o que tira o silencio e este numero no `replicacao_estado`, ao lado da
    /// linha no log. Conta criacao, e nao abertura: a cadeia ja criada abre a
    /// cada rodada, e contar a abertura inflaria o numero sem tabela nova.
    ledger_marcado_recebido: AtomicU64,
    /// Pedido 652: quais pares ja foram avisados de que o Noise sera recusado
    /// na 0.20. Silencio por par; consultado so em `responder_aperto`.
    aviso_do_noise: crate::fio_dados::AvisoDoNoise,
    /// Os outros dois ajustes que a tela de configuracao muda A QUENTE.
    ///
    /// O `somente_leitura_vivo` acima serve aos DOIS caminhos que o mudam: a
    /// promocao de um spare e a gravacao pela tela. Sao a mesma pergunta --
    /// "este servidor aceita escrita AGORA?" -- e dois campos para ela
    /// virariam duas respostas no dia em que um caminho esquecesse o outro.
    max_linhas_vivo: AtomicU64,
    espelho_vivo: AtomicBool,
    /// A privada estatica do aperto de mao da porta de dados.
    ///
    /// Preguicosa DE PROPOSITO: nasce na primeira vez que alguem pede o
    /// aperto, e nao no arranque. Um servidor com quem ninguem faz aperto nao
    /// passa a escrever um arquivo que antes nao escrevia -- que e a regra do
    /// "guarda nova entra pedida" aplicada ao disco.
    estatica_do_fio: Mutex<Option<[u8; 32]>>,
    /// A identidade TLS da porta de dados, lida no `escutar` (pedido 572,
    /// T6). `None` = a porta so fala claro, como sempre falou.
    tls_dos_dados: Mutex<Option<Arc<phxsql_core::tls::Identidade>>>,
    /// O que o servidor esta fazendo AGORA: atividades, threads e as series.
    ///
    /// `Arc` porque as threads de fundo carregam o registro consigo para
    /// anotar o que estao fazendo, e elas vivem mais que qualquer emprestimo.
    telemetria: Arc<crate::telemetria::Telemetria>,
    /// O cadastro de usuarios VIVO -- nasce igual ao do `config.json` e muda
    /// pelas tres operacoes do pedido 221 (`usuario_criar`, `usuario_alterar`,
    /// `usuario_excluir`).
    ///
    /// # Por que ele nao mora no `Config`
    ///
    /// Porque o `Config` e a fotografia do arquivo no arranque, e o resto do
    /// servidor conta com isso -- ele e emprestado por `&self` sem trava
    /// nenhuma, em quarenta lugares. O cadastro e a unica parte dele que muda
    /// em vida, e por isso e a unica que paga uma trava. Quem le o cadastro
    /// passa a chamar `cadastro()`; quem le o resto do `Config` nao mudou uma
    /// linha.
    cadastro_vivo: RwLock<crate::usuarios::Cadastro>,
    /// Quantas vezes o cadastro mudou. Zero = nunca mexeram nele.
    ///
    /// # O portao que vem ANTES do trabalho
    ///
    /// A mesma decisao do `profiler_ligado` e do `transacoes_abertas`: um
    /// `load(Relaxed)` decide se ha o que fazer, e num servidor onde ninguem
    /// chamou `usuario_criar` a conta acaba ali -- nenhuma trava tomada,
    /// nenhuma `String` montada, nenhuma busca no cadastro. So depois de a
    /// primeira mudanca acontecer e que cada conexao paga UMA releitura da
    /// propria ficha, e nunca mais ate a mudanca seguinte.
    cadastro_geracao: AtomicU64,
}

impl Servidor {
    /// Toma a trava unica de dados -- e e o UNICO lugar que a toma.
    ///
    /// # Por que passar por aqui
    ///
    /// A trava de dados e o gargalo declarado deste servidor: toda leitura e
    /// toda escrita passam por ela, uma de cada vez. Entao «quanto tempo se
    /// espera por ela» e «quanto tempo alguem a segura» sao os dois numeros
    /// que explicam um servidor lento -- e nenhum dos dois existia.
    ///
    /// Medir em cada tomada seria copiar a mesma conta dezenas de vezes, e a
    /// que alguem esquecesse viraria o buraco na serie -- a mesma razao pela
    /// qual o portao de permissao e UM so.
    ///
    /// # O «unico» ja foi mentira, e o teste que o segura
    ///
    /// Esta frase esteve errada por rodadas: havia 13 `self.dados.lock()`
    /// fora daqui, e tres delas doiam -- o despejo do cache, o corpo de um
    /// gatilho e o laco da replicacao, este ultimo segurando a trava atraves
    /// de uma ida e volta de rede. As 13 entraram; `so_um_lugar_toma_a_trava`
    /// conta as ocorrencias no proprio fonte e reprova a decima-quarta.
    /// Comentario que se afirma unico precisa de quem conte.
    ///
    /// # A reentrancia, que era um servidor pendurado
    ///
    /// `std::sync::Mutex` nao e reentrante: pedir de novo a trava que esta na
    /// mao DESTA thread parava o servidor inteiro, para sempre, sem log e sem
    /// pilha. Aconteceu tres vezes neste projeto. A `COM_A_TRAVA` transforma
    /// isso num erro comum: o pedido culpado falha dizendo o que houve, e os
    /// outros continuam sendo atendidos.
    ///
    /// # O que custa
    ///
    /// Ligada: dois `Instant::now()` por OPERACAO (nao por linha), num
    /// caminho em que a operacao mais barata ja leva dezenas de
    /// microssegundos. Desligada: um `load(Relaxed)` e duas visitas a uma
    /// `Cell` de thread, e nem o relogio e lido.
    fn travar_dados(&self) -> Result<TravaMedida<'_>> {
        // ANTES de qualquer trabalho, e antes de parar na fila: se esta thread
        // ja tem a trava, esperar por ela e esperar por si mesma.
        if COM_A_TRAVA.with(std::cell::Cell::get) {
            return Err(trava_reentrante());
        }
        let medindo = self.telemetria.ligada();
        let atividade = if medindo {
            crate::telemetria::corrente()
        } else {
            None
        };
        // O estado muda ANTES de a thread parar na fila: e essa marca que faz
        // a bolha aparecer amarela «esperando» enquanto outra atividade
        // segura a trava. Depois seria tarde -- a thread ja estaria bloqueada.
        if let Some(a) = &atividade {
            a.esperando_trava();
        }
        let pedida = medindo.then(Instant::now);
        // O portao do retrato ANTES da fila do `RwLock` (pedido 513): com um
        // backup copiando, o escritor dorme aqui e nao na fila -- la ele
        // faria todo leitor novo esperar junto. Dentro do cronometro de
        // proposito: para quem pediu, e espera pela trava do mesmo jeito.
        let passagem = self.retrato.passar();
        // O veneno so passa quando o reparo o alcancou -- ver
        // `panicos_na_trava` e o `TravaMedida::drop`, pedido 451.
        let guarda = match self.dados.write() {
            Ok(g) => Ok(g),
            Err(veneno) => self.depois_do_veneno(veneno),
        };
        // UM relogio para as duas contas: o instante em que a trava chegou na
        // mao e o fim da espera e o comeco da posse. Ler o relogio duas vezes
        // aqui pagaria duas chamadas para saber a mesma coisa.
        let obtida = medindo.then(Instant::now);
        if let Some(a) = &atividade {
            a.com_a_trava();
        }
        if let (Some(t), Some(o)) = (pedida, obtida) {
            self.telemetria
                .contar_espera(o.duration_since(t).as_micros() as u64);
        }
        // So depois do `?`: trava envenenada nao chegou a ser tomada, e uma
        // marca deixada aqui trancaria esta thread para o resto da vida dela.
        let mut guarda = guarda?;
        // A ficha chegou na mao: e isto que o leitor em laco espera ver andar
        // quando cede a vez (pedido 623, `PortaoDoRetrato::ceder`).
        passagem.entrou();
        COM_A_TRAVA.with(|c| c.set(true));
        // Uma tomada da trava de escrita e UMA transacao no diario (pedido
        // 676): todo evento gravado ate o `Drop` leva o mesmo id, e e com ele
        // que a replica aplica o commit de varias tabelas inteiro. Aqui, e
        // so aqui, pelo mesmo motivo do reparo e do quorum: e o unico lugar
        // que toma a trava de escrita. Custa uma `Cell` de thread.
        phxsql_store::log::abrir_unidade();
        // O quorum de escrita (pedido 207): o portao e uma leitura atomica e
        // vem ANTES de qualquer trabalho. Desligado, nenhum evento do diario
        // paga mais que a leitura de uma `Cell` de thread.
        // E a mesma anotacao serve ao backup em duas passadas (pedido 513,
        // passo 2): com retrato ligado, as tabelas tocadas sao a terceira
        // rede da fase 2. Dois `load` quando nenhum dos dois esta ligado.
        if self.quorum_vale_aqui() || phxsql_store::congelamento::em_retrato() {
            phxsql_store::log::anotar_tocadas();
        }
        // A ficha sai da vida do emprestimo e passa a viver ao lado do guard,
        // no mesmo `struct` -- ver `Exclusiva::sem_amarra`. Os dois morrem
        // juntos, no `Drop` daqui.
        let instancia = guarda.exclusiva().sem_amarra();
        Ok(TravaMedida {
            guarda,
            instancia,
            tomada: obtida,
            servidor: self,
            tomada_no_desenrolar: std::thread::panicking(),
            marca_em_voo: None,
            _passagem: passagem,
        })
    }

    /// A trava de dados achada ENVENENADA: volta a servir so se o reparo ja
    /// alcancou todo panico que a sujou -- pedido 451.
    ///
    /// # Por que nao o motor da `TravaDaGuarda`
    ///
    /// Porque a pergunta e outra. A `TravaDaGuarda` (pedidos 436 e 447)
    /// recupera SEMPRE, e esta certa em recuperar: o que ela guarda e
    /// `HashMap` e `Vec`, que o desenrolar nao entorta. Aqui o que se entorta
    /// e o DISCO, e recuperar sem reparar e a H2 ingenua que o parecer do DBA
    /// reprovou -- escrever por cima de uma arvore rasgada e de uma marca
    /// orfa. A decisao «recupera quando o reparo terminou» e desta trava so, e
    /// mora num lugar so: as duas portas (`travar_dados` e
    /// `travar_dados_para_ler`) chamam esta.
    ///
    /// # O aviso
    ///
    /// Sai UMA vez por panico, e nao a cada tomada: quem o escreve e o proprio
    /// reparo, com o relatorio do que fez. Esta funcao fica calada no caminho
    /// que recupera -- o veneno e permanente (de proposito: ver o campo
    /// `panicos_na_trava`, pedido 653), e um aviso por tomada seria uma linha
    /// por pedido, o aviso que ninguem le.
    fn depois_do_veneno<G>(&self, veneno: std::sync::PoisonError<G>) -> Result<G> {
        let panicos = self.panicos_na_trava.load(Ordering::SeqCst);
        let reparos = self.reparos_da_trava.load(Ordering::SeqCst);
        if panicos > 0 && reparos == panicos {
            return Ok(veneno.into_inner());
        }
        Err(trava_de_dados_sem_reparo(panicos, reparos))
    }

    /// Toma a trava de dados PARA LER -- e e o unico lugar que a toma assim.
    ///
    /// # O que ela compra, e o que ela nao muda
    ///
    /// Leitor deixa de esperar leitor. Nada mais: escritor continua exclusivo,
    /// e continua excluindo os leitores. O ganho medido esta no
    /// `docs/CONCORRENCIA.md`.
    ///
    /// # Por que ela e uma porta SEPARADA, e nao um parametro
    ///
    /// Porque o que ela entrega e outro tipo. `travar_dados()` devolve a ficha
    /// exclusiva, por onde se grava; esta devolve a [`Raiz`] emprestada em
    /// modo compartilhado, e por ela so se alcanca tabela de LEITURA. Um
    /// `travar_dados(modo)` devolveria o mesmo tipo nos dois casos, e ai a
    /// garantia voltaria a ser convencao.
    ///
    /// # A reentrancia vale igual, e por um motivo pior
    ///
    /// `std::sync::RwLock` nao e reentrante e a segunda tomada NAO e so
    /// esperar por si mesmo: com um escritor na fila, pedir a leitura de novo
    /// enquanto se tem a leitura na mao trava as tres pontas. A `COM_A_TRAVA`
    /// e a mesma dos dois lados de propósito -- misturar as duas fichas na
    /// mesma thread e o mesmo defeito, com nome diferente.
    ///
    /// # A RAM, que a trava segurava de graca
    ///
    /// O cache de paginas do `.ndx` e por `Table` ABERTA -- 2.048 paginas de
    /// 4 KiB, 8 MiB de teto cada. O comentario do `ndx.rs` justifica esse teto
    /// dizendo que «a tabela abre e fecha a cada operacao, entao o teto vale
    /// enquanto a operacao dura», e isso era verdade **porque a trava
    /// serializava tudo**. Aqui deixa de ser: N leitores sao N caches.
    ///
    /// Medido (`--example quanto-cache-uma-leitura-usa`): a grade SEM ordem
    /// paga **zero** -- ela percorre o `.reg` e nao toca o indice --, e a
    /// ORDENADA paga **6,52 MiB**, com 50 linhas custando o mesmo que 1.000.
    /// Oito leitores ordenados ao mesmo tempo dao 52,1 MiB. O teto segura (o
    /// cache despeja e troca RAM por releitura), e quem mexer em
    /// `recursos.cache_paginas` daqui em diante esta mexendo num numero que se
    /// multiplica por leitor. Ver `docs/CONCORRENCIA.md` §16.7.
    ///
    /// # O tempo entra na MESMA serie da trava
    ///
    /// Podia ter serie propria, e nao tem: a serie existe para responder «por
    /// que este servidor esta lento», e uma operacao que sumisse dela levaria
    /// junto a resposta. O que se perde e distinguir posse compartilhada de
    /// exclusiva dentro do numero; o que se ganharia com duas series e um
    /// buraco no dia em que alguem esquecesse de somar as duas.
    fn travar_dados_para_ler(&self) -> Result<TravaDeLeitura<'_>> {
        if COM_A_TRAVA.with(std::cell::Cell::get) {
            return Err(trava_reentrante());
        }
        let medindo = self.telemetria.ligada();
        let atividade = if medindo {
            crate::telemetria::corrente()
        } else {
            None
        };
        if let Some(a) = &atividade {
            a.esperando_trava();
        }
        let pedida = medindo.then(Instant::now);
        // O leitor que entra em panico NAO envenena (regra do `RwLock` da
        // `std`), mas acha o veneno do escritor -- e passa pelo MESMO portao
        // da ficha exclusiva, senao a leitura continuaria trancada depois de
        // o reparo terminar.
        let guarda = match self.dados.read() {
            Ok(g) => Ok(g),
            Err(veneno) => self.depois_do_veneno(veneno),
        };
        let obtida = medindo.then(Instant::now);
        if let Some(a) = &atividade {
            a.com_a_trava();
        }
        if let (Some(t), Some(o)) = (pedida, obtida) {
            self.telemetria
                .contar_espera(o.duration_since(t).as_micros() as u64);
        }
        let guarda = guarda?;
        COM_A_TRAVA.with(|c| c.set(true));
        Ok(TravaDeLeitura {
            guarda,
            tomada: obtida,
            telemetria: &self.telemetria,
        })
    }

    /// Le o pedido e o leva pelos portoes, nesta ordem: politica (o que ninguem
    /// pode), token (a rede), login (a identidade) e permissao (o poder).
    fn despachar(
        &self,
        linha: &str,
        sessao: &mut Sessao,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        // O quorum do pedido (207) nasce limpo: o de um pedido anterior desta
        // thread nao pode vazar para a resposta deste.
        QUORUM_DO_PEDIDO.with(|q| q.borrow_mut().take());
        let (op, autenticado, mut resultado) = self.despachar_o_pedido(linha, sessao, ip);
        // A espera aconteceu no `Drop` da trava, la dentro; a resposta sai
        // daqui. Sem quorum ligado nada foi guardado, e a resposta e byte a
        // byte a de sempre (teste 9 do contrato, o comportamento velho).
        if let Some(q) = QUORUM_DO_PEDIDO.with(|q| q.borrow_mut().take()) {
            if let Ok(j) = &mut resultado {
                self.por_o_quorum_na_resposta(j, &q);
            }
        }
        (op, autenticado, resultado)
    }

    /// O campo `quorum` da resposta, e o aviso quando ele NAO foi alcancado
    /// -- o gesto do `syncrep.c:321` do PostgreSQL: a gravacao ficou, e quem
    /// pediu fica sabendo que ela pode nao ter chegado a replica nenhuma.
    fn por_o_quorum_na_resposta(&self, j: &mut Json, q: &crate::quorum::Resultado) {
        // Resposta que nao e objeto (uma lista, um numero) nao ganha campo:
        // embrulha-la mudaria a forma que o cliente de hoje le. Toda escrita
        // do protocolo responde objeto -- a guarda `quorum-escritor-sem-espera`
        // confere as familias.
        if !matches!(j, Json::Objeto(_)) {
            return;
        }
        j.definir("quorum", q.para_json());
        if !q.alcancado {
            j.definir(
                "aviso_quorum",
                Json::texto_de(self.msg(
                    "erro.quorum_nao_alcancado",
                    &[
                        ("pedido", &q.pedido.to_string()),
                        ("confirmado", &q.confirmado.to_string()),
                    ],
                )),
            );
        }
    }

    fn despachar_o_pedido(
        &self,
        linha: &str,
        sessao: &mut Sessao,
        ip: &str,
    ) -> (String, bool, Result<Json>) {
        let pedido = match Json::analisar(linha) {
            Ok(p) => p,
            Err(e) => return ("?".into(), false, Err(e)),
        };
        let op = pedido.texto_ou("op", "").trim().to_string();
        let op = if op.is_empty() {
            "ping".to_string()
        } else {
            op
        };
        #[cfg(test)]
        self.armar_panico_de_teste(&op);
        let base = pedido.texto_ou("database", "").to_string();

        // Portao 0 -- a politica. Vale para todo mundo, root inclusive: e o
        // que o config.json diz que ninguem pede por esta porta. Pedir vira
        // violacao grave: bloqueio na hora com a politica padrao, ou na
        // enesima quando `tentativas_para_bloqueio` diz para tolerar --
        // e a recusa da operacao acontece SEMPRE, desde a primeira.
        if self.config.politica.comando_proibido(&op) {
            let destino = self.violacao_grave(ip, &op, "comando proibido pela politica");
            return (
                op.clone(),
                false,
                Err(PhxError::Autorizacao(self.recado_de_grave(
                    &destino,
                    "erro.comando_proibido",
                    &[("op", op.as_str())],
                ))),
            );
        }
        // Nome com ".." ou barra nao e engano de digitacao: e sondagem de
        // travessia de diretorio. O motor ja recusava -- mas recusava calado, e
        // quem sonda podia tentar a noite inteira sem nunca ser barrado. E
        // violacao grave, igual a comando proibido, com a mesma tolerancia
        // configuravel -- regra unica, para o operador nao ter de decorar qual
        // grave conta e qual nao conta.
        for (rotulo, valor) in [
            ("database", &base),
            ("tabela", &pedido.texto_ou("tabela", "").to_string()),
            ("schema", &pedido.texto_ou("schema", "").to_string()),
        ] {
            if !valor.is_empty() && phxsql_store::catalogo::nome_hostil(valor) {
                let destino = self.violacao_grave(ip, &op, "tentativa de travessia de diretorio");
                return (
                    op,
                    false,
                    Err(PhxError::Autorizacao(self.recado_de_grave(
                        &destino,
                        "erro.nome_hostil",
                        &[("rotulo", rotulo), ("valor", &format!("{valor:?}"))],
                    ))),
                );
            }
        }

        // A proibicao POR BANCO, no MESMO portao: o pedido 220 pedia «para um
        // banco x», e a resposta certa nao e um segundo portao ao lado deste
        // -- e a mesma conferencia, um campo depois. Vem depois da global de
        // proposito: o global vale para o pedido que nem nomeia banco.
        //
        // A bandeira atomica antes do `lock` e o que faz esta guarda custar
        // zero para quem nunca a pediu.
        if self.ha_proibidos_por_base.load(Ordering::Relaxed)
            && !base.is_empty()
            && self
                .proibidos_por_base
                .lock()
                .map(|l| {
                    let alvo = op.trim().to_lowercase();
                    l.iter().any(|(b, c)| *b == base && *c == alvo)
                })
                .unwrap_or(false)
        {
            let destino = self.violacao_grave(ip, &op, "comando proibido neste banco");
            return (
                op.clone(),
                false,
                Err(PhxError::Autorizacao(self.recado_de_grave(
                    &destino,
                    "erro.comando_proibido_na_base",
                    &[("op", op.as_str()), ("base", base.as_str())],
                ))),
            );
        }

        if self.config.politica.base_proibida(&base) {
            let destino = self.violacao_grave(ip, &op, "base proibida pela politica");
            return (
                op,
                false,
                Err(PhxError::Autorizacao(self.recado_de_grave(
                    &destino,
                    "erro.base_proibida",
                    &[("base", base.as_str())],
                ))),
            );
        }

        // Portao 1 -- o token. E a chave da porta da rede, nao a identidade.
        if !self.config.token_confere(pedido.texto_ou("token", "")) {
            self.violacao_leve(ip, &op, "token invalido");
            return (
                op,
                false,
                Err(PhxError::Autorizacao(self.msg("erro.token_invalido", &[]))),
            );
        }

        // Portao 2 -- o login.
        if op == "desafio" {
            let r = self.op_desafio(&pedido, sessao);
            return (op, true, r);
        }
        if op == "login" {
            let r = self.op_login(&pedido, sessao);
            if r.is_err() {
                self.violacao_leve(ip, "login", "credencial invalida");
            }
            return (op, r.is_ok(), r);
        }
        // Sair nao precisa de poder nenhum: e devolver o que se tem.
        if op == "sair" {
            sessao.usuario = None;
            sessao.desafio = None;
            return (op, true, Ok(Json::objeto(vec![("saiu", Json::Bool(true))])));
        }
        // A ficha da conexao acompanha o cadastro VIVO.
        //
        // Vem DEPOIS do `sair` e ANTES do portao do login, que e a unica
        // ordem que fecha o caso: quem foi excluido no meio da conexao perde
        // a ficha aqui e cai no «faca login» logo abaixo, com o erro que todo
        // cliente ja sabe tratar -- e nao com um reset de soquete, que a
        // aplicacao do outro lado leria como falha de rede.
        self.refrescar_a_sessao(sessao);
        if self.ainda_anonima(sessao) && pede_identidade(&op, &pedido) {
            return (
                op,
                true,
                Err(PhxError::Autorizacao(self.msg("erro.faca_login", &[]))),
            );
        }

        // Portoes 2b, 3 e 4 -- ver `portoes_do_pedido`.
        if let Err(e) = self.portoes_do_pedido(&op, &pedido, sessao) {
            return (op, true, Err(e));
        }

        // O direito por COLUNA embrulha o `executar` -- ver
        // `aplicar_direito_por_coluna`, que explica por que ele nao cabe num
        // portao. Sem regra de coluna no cadastro, e uma leitura de `bool`. E
        // a escrita local na replica se conta so DEPOIS dele, com `Ok`
        // (pedido 630).
        let r = self.executar_e_contar_escrita_local(&op, &pedido, sessao);

        // Pedido 215 -- a injecao de SQL que ninguem bloqueava.
        //
        // O interruptor vem ANTES do trabalho, e a ordem e a licao do
        // Profiler: desligado (o padrao) isto custa a leitura de um `bool`, e
        // nao um `Json::texto_ou` mais uma varredura lexica por pedido
        // recusado. Medido antes: 172.620 tentativas por minuto com uma
        // conexao nova a cada uma, `blacklist.json` vazio antes e depois.
        //
        // Nao ha portao novo: quem conta e o `violacao_leve` de sempre, com o
        // `tentativas_ate_bloquear` e a `janela_minutos` da mesma politica --
        // o mesmo caminho do token invalido e da credencial errada.
        if self.config.politica.contar_injecao_sql
            && r.is_err()
            && op == "sql"
            && !ip.is_empty()
            && phxsql_sql::comando_empilhado(pedido.texto_ou("texto", pedido.texto_ou("sql", "")))
        {
            self.violacao_leve(ip, &op, "comando SQL empilhado");
        }
        (op, true, r)
    }

    /// Releva a ficha desta conexao contra o cadastro vivo, quando ele mudou.
    ///
    /// # Por que a marca de geracao, e nao reler sempre
    ///
    /// Porque reler sempre custaria uma trava e uma busca por pedido para o
    /// servidor inteiro, e num servidor em que ninguem nunca chamou
    /// `usuario_criar` a resposta seria sempre a mesma. E o portao que vem
    /// ANTES do trabalho: um `load(Relaxed)` compara duas geracoes, e quando
    /// elas batem -- que e o caso de todo pedido de todo servidor que nunca
    /// mexeu no cadastro -- nao ha trava, busca nem `String`.
    ///
    /// # E por que a sessao nao morre junto com o usuario
    ///
    /// Porque derrubar a conexao seria a aplicacao do outro lado recebendo um
    /// erro de REDE por uma decisao de CADASTRO. Aqui ela recebe o mesmo
    /// «faca login» que receberia se nunca tivesse entrado, que e um erro que
    /// todo cliente ja trata -- e a diferenca aparece exatamente no cliente
    /// mais antigo, que e quem menos sabe se recuperar.
    fn refrescar_a_sessao(&self, sessao: &mut Sessao) {
        let agora = self.cadastro_geracao.load(Ordering::Relaxed);
        if agora == sessao.geracao_do_cadastro || sessao.usuario.is_none() {
            return;
        }
        sessao.geracao_do_cadastro = agora;
        let login = sessao.login().to_string();
        sessao.usuario = self
            .cadastro()
            .por_login(&login)
            .filter(|u| u.ativo)
            .cloned();
    }

    /// O portao de administracao, para as operacoes que nao nomeiam base.
    ///
    /// # Por que ele existe alem do portao geral
    ///
    /// O portao do `despachar` confere o campo `"tabela"` e cai na regra da
    /// base VAZIA quando ele nao existe. Isso ja exige `administrar` -- mas
    /// deixaria as guardas mais importantes do servidor amarradas a um
    /// detalhe de resolucao de nome de base. Aqui a conferencia e explicita.
    ///
    /// E ele e UM: o `config_gravar` e as tres operacoes de cadastro chamam
    /// esta funcao. Uma copia em cada operacao seria a porta dos fundos de
    /// sempre -- a copia que alguem esquecesse de atualizar.
    fn exigir_administrar(&self, sessao: &Sessao, oque: &str) -> Result<()> {
        if let Some(u) = &sessao.usuario {
            if !u.pode_em("", "", Atividade::Administrar) {
                return Err(PhxError::Autorizacao(format!(
                    "{} nao tem permissao de administrar: {oque} exige esse poder",
                    u.login
                )));
            }
        }
        Ok(())
    }

    /// Os portoes que valem para QUALQUER origem, e nao so para a rede.
    ///
    /// Estao juntos aqui porque o portao tem de ser UM. O agendador de jobs
    /// nao chega por soquete -- nao passa pelo token nem pela lista negra --,
    /// mas o somente-leitura, o poder do usuario sobre a base E A TABELA e a
    /// reserva de carga valem para ele igual. Escrever essa conferencia num
    /// segundo lugar e exatamente como a porta dos fundos aparece: a copia que
    /// alguem esquecer de atualizar vira o furo, e ninguem acha por leitura.
    ///
    /// O que NAO esta aqui e o que so faz sentido com um IP do outro lado: a
    /// politica de comando proibido bloqueia quem pediu, e bloquear "o
    /// agendador" nao quer dizer nada. Quem chama de outra origem confere a
    /// politica por conta, e o comentario de `rodar_job` diz como.
    fn portoes_do_pedido(&self, op: &str, pedido: &Json, sessao: &Sessao) -> Result<()> {
        // Portao 2a -- o PAPEL do servidor. Antes do somente-leitura, para a
        // recusa dizer o que importa: nao e "voce nao pode", e "este servidor
        // nao atende isso -- o primario e ali". Um portao so, aqui, e nao
        // espalhado pelas operacoes: a que alguem esquecesse viraria a porta
        // dos fundos.
        match self.papel_atual() {
            Papel::Spare if !OPS_NO_SPARE.contains(&op) => {
                return Err(PhxError::SpareEmEspera(format!(
                    "este servidor e um spare de contingencia e nao atende \
                     cliente (nem leitura); o primario e {}. Para assumir o \
                     trabalho: {{\"op\":\"spare_promover\"}}",
                    self.primario()
                )));
            }
            // O mesmo erro do redirecionamento de cluster, e de proposito: para
            // o cliente, "escreveu no no errado, va para aquele" e UM evento so.
            // O `REDIRECIONA host:porta` na frente e o pedaco que ele recorta.
            Papel::ReadReplica if grava_dado_replicado(op) => {
                return Err(PhxError::Redireciona(format!(
                    "REDIRECIONA {} -- este servidor e uma replica de leitura; \
                     escreva no primario",
                    self.primario()
                )));
            }
            _ => {}
        }

        // Portao 2a-bis -- de ONDE a replicacao pode vir.
        //
        // `replicacao.replicas_autorizadas` existia no `config.json`, na
        // secao 7 do REPLICACAO.md e na tela de configuracao, e NENHUMA linha
        // de codigo o lia. A bancada de conteiner mediu o estrago: um vizinho
        // de rede com um `config.json` de replica vazado -- mesmo token,
        // mesmo usuario, mesmo `senha_hash` -- levou os 200 de 200 eventos do
        // diario COM a lista preenchida. Configuracao que nao e lida mente, e
        // mente pior quando o assunto e quem alcanca o dado.
        //
        // PEDIDA, NAO IMPOSTA: lista vazia -- o padrao, e o que todo
        // `config.json` de hoje tem -- libera todos, byte a byte o
        // comportamento de sempre. Quem preenche a lista ganha a garantia.
        //
        // O campo que este portao passou a olhar e o IP DA SESSAO, entao a
        // pergunta obrigatoria e quem NAO tem esse campo: job agendado,
        // rotina interna e a replicacao chamada de dentro chegam com `ip`
        // vazio -- e vazio ali e a verdade, nao uma falta. Nao vieram de
        // fora, nao ha IP para autorizar, e o portao nao se aplica a eles.
        // Um portao so, aqui, e nao espalhado pelas tres operacoes.
        if OPS_DE_REPLICACAO.contains(&op) && !sessao.ip.is_empty() {
            let lista = &self.config.replicacao.replicas_autorizadas;
            // Atras do proxy declarado o IP e o do PROXY (pedido 284): com
            // `["127.0.0.1"]` na lista, todo cliente que chega por ele seria
            // a replica autorizada. A lista nao tem como decidir ali, entao
            // a porta HTTP com proxy recusa a replicacao -- a replica tem a
            // porta de dados, onde o par do soquete e ela. Lista vazia nao
            // entra aqui: guarda pedida, o comportamento de sempre fica.
            //
            // SEM `violacao_leve`, e de proposito: o IP aqui e o do proxy, e
            // contar violacao contra ele levaria a lista negra a barrar TODO
            // cliente que chega pelo proxy por causa de um. A recusa continua
            // no `acessos.log`, que o chamador escreve.
            if !lista.is_empty() && sessao.ip_do_proxy {
                return Err(PhxError::Autorizacao(
                    self.msg("erro.replica_atras_de_proxy", &[]),
                ));
            }
            if !lista.is_empty() && !lista.iter().any(|p| p == &sessao.ip) {
                self.violacao_leve(&sessao.ip, op, "ip fora de replicas_autorizadas");
                return Err(PhxError::Autorizacao(
                    self.msg("erro.replica_nao_autorizada", &[]),
                ));
            }
        }

        // Portao 2b -- a escrita. Com cluster, quem decide e o papel VIVO dele:
        // a replica redireciona para o master e um master sem maioria visivel
        // recusa, para conter o split-brain. SEM cluster, vale o
        // `somente_leitura` VIVO, que a promocao de um spare abre sem
        // reiniciar. A ordem importa: um no de cluster carrega
        // `somente_leitura: true` como replica, e conferir isso ANTES do
        // cluster faria a promocao nao promover nada.
        //
        // A recusa do somente-leitura sai pela tabela de mensagens (texto que
        // gente le, entao acompanha o idioma); a do cluster ja vem pronta.
        //
        // A pergunta e «grava o dado replicado?», e nao «escreve?»: o que so
        // mexe no arquivo deste no (`OPS_DO_NO`) passa, porque e o
        // administrador DESTE no que o alcanca -- pedidos 368 e 499.
        if grava_dado_replicado(op) {
            if let Some(estado) = &self.cluster {
                if let Some(recusa) = estado.recusa_de_escrita() {
                    return Err(recusa);
                }
            } else if self.somente_leitura() {
                // Le o valor VIVO, que dois caminhos escrevem: a promocao de um
                // spare e a gravacao pela tela de configuracao.
                return Err(PhxError::Autorizacao(self.msg("erro.somente_leitura", &[])));
            } else if let Some(origem) = self.origem_que_traz(pedido.texto_ou("database", "")) {
                // Pedido 677: um escritor por database. O que vem de uma
                // origem por replicacao e so leitura aqui, mesmo sem
                // `somente_leitura` -- que tranca o servidor inteiro, e no
                // espelho do 325 o caixa PRECISA escrever no database dele.
                return Err(PhxError::Autorizacao(self.msg(
                    "erro.base_recebida_por_replica",
                    &[
                        ("base", pedido.texto_ou("database", "")),
                        ("origem", origem.nome.as_str()),
                        ("onde", &format!("{}:{}", origem.host, origem.porta)),
                    ],
                )));
            }
            // A escrita local que passou daqui NAO se conta aqui (pedido 630):
            // os portoes 3 a 5 ainda nao julgaram, e a tabela nem abriu. A
            // conta mora em `executar_e_contar_escrita_local`.
        }

        // Portao 2b-bis -- `aplicar` num servidor trancado por ADMINISTRACAO.
        //
        // `aplicar` esta fora de `OPS_ESCRITA` de proposito (o comentario da
        // lista diz por que), e a consequencia media pela bateria
        // `bancada/seguranca/porta.py` (caso 4d-ii) e que num SOURCE trancado
        // o `inserir` recusava e o `aplicar` gravava a mesma linha na mesma
        // sessao. Quem tem o token grava numa base que o dono declarou
        // fechada.
        //
        // # O que distingue o legitimo do furo, e como foi medido
        //
        // Uma replica de verdade roda em `somente_leitura` POR DESENHO (o
        // `Config_exemplo_03.json` e o `montar.py` da bancada), entao barrar
        // `aplicar` por `somente_leitura` e so quebraria a replicacao. Medido
        // antes de decidir, com um source e uma replica de pe (papel
        // `replica`, `somente_leitura` ligado, 200 linhas alcancadas): a op
        // `aplicar` foi chamada ZERO vezes nos dois `acessos.log` -- o laco da
        // replica puxa e aplica por dentro, com `Table::aplicar_evento`, e
        // nunca pelo protocolo. Quem chama `aplicar` pela rede e um EMPURRAO
        // de fora, e o unico servidor que tem motivo para aceita-lo trancado e
        // o que existe para receber replicacao.
        //
        // Por isso o crivo e o PAPEL, e nao a lista de replicas nem a
        // existencia de origens: papel e a declaracao de para que este
        // servidor serve. Source e isolado recusam; replica, read_replica,
        // spare e multi continuam byte a byte como antes.
        //
        // # E o crivo vale ABERTO ou trancado -- desde 17/09/2026
        //
        // Ate entao ele so rodava com `somente_leitura`, porque nasceu do caso
        // 4d-ii da bateria, que era um source TRANCADO. Mas `aplicar` grava
        // com `Table::aplicar_evento`, que desliga o julgamento de integridade
        // de proposito (a garantia e da origem, e conferir na replica perdia
        // dado nos tres ordenamentos -- esta medido em `table.rs`): sem chave
        // estrangeira, sem CHECK, sem cascata. Num source ABERTO -- que e o
        // source de producao, por definicao -- quem tem `administrar` na
        // tabela apagava pela rede o pai com filhos, contornando a regra
        // primordial da integridade por uma operacao que a declaracao da
        // tabela nunca viu (revisao SEC, A3). O crivo e sobre o que a
        // operacao FAZ, e o que ela faz nao depende de o servidor estar
        // trancado. Medido antes de mexer: nenhum chamador legitimo empurra
        // `aplicar` num source ou isolado -- as bancadas de quorum empurram
        // em REPLICAS, e a de seguranca e a prova deste portao.
        //
        // O `cluster.is_none()` preserva o cluster inteiro: la quem decide
        // escrita e o papel VIVO da eleicao, e uma segunda regra por cima seria
        // a copia que alguem esquece de atualizar.
        if op == "aplicar" && self.cluster.is_none() {
            let papel = self.papel_atual();
            let recebe_replicacao = matches!(
                papel,
                Papel::Replica | Papel::ReadReplica | Papel::Spare | Papel::Multi
            );
            if !recebe_replicacao {
                return Err(PhxError::Autorizacao(
                    self.msg("erro.aplicar_fora_de_replica", &[("papel", papel.nome())]),
                ));
            }
        }

        // Portao 3 -- o poder deste usuario sobre a base E A TABELA do pedido.
        //
        // A tabela entra aqui, e nao la dentro de cada operacao, porque o
        // portao tem de ser UM: espalhado por quarenta operacoes, a que
        // alguem esquecer de conferir vira a porta dos fundos, e ninguem
        // descobre por leitura.
        //
        // Pedido sem tabela -- `bancos`, `criar_database`, `sistema` -- cai na
        // regra da base, que e como sempre foi.
        let base = pedido.texto_ou("database", "").to_string();
        if let (Some(atividade), Some(usuario)) =
            (Atividade::da_operacao(op), sessao.usuario.as_ref())
        {
            let tabela = pedido.texto_ou("tabela", "").trim().to_string();
            if !usuario.pode_em(&base, &tabela, atividade) {
                return Err(self.recusa_sem_direito(usuario, atividade, &base, &tabela));
            }
        }

        // Portao 4 -- a tabela esta reservada para uma carga de outra ligacao?
        //
        // Depois do de permissao, e nao antes: quem nao pode nem ler a tabela
        // nao precisa descobrir que ela esta em carga, e o recado diz QUEM
        // reservou. `bulkinsert` fica de fora para o comando dizer o proprio
        // recado -- e para o administrador conseguir soltar a reserva alheia.
        //
        // # O campo que este portao le, e quem NAO tem esse campo (pedido 322)
        //
        // Ele lia so `"tabela"`, e a reserva se contornava pedindo a tabela
        // em carga como o lado B de um `juntar` -- ou em `diferencas`, numa
        // lista de `unir`, num `juntar` aninhado do `pivotar`, ou como
        // `destino` de uma copia. A lista de onde a tabela se esconde ja
        // existe, e e UMA: `direito_coluna::tabelas_do_pedido`. Uma segunda
        // copia dela aqui seria a lista que alguem esquece de atualizar no dia
        // em que entrar a proxima operacao com a tabela fora do campo.
        if op != "bulkinsert" {
            if let Some(recado) = self.barrado_por_carga(op, pedido, sessao.ligacao) {
                return Err(PhxError::EmCarga(recado));
            }
        }

        // Portao 5 -- a tabela esta segurada pela TRANSACAO de outra conexao?
        //
        // O primeiro `if` e um `load(Relaxed)` num atomico: sem transacao
        // aberta em lugar nenhum -- o servidor de hoje -- nada aqui custa.
        //
        // # O campo que este portao le, e quem NAO tem esse campo
        //
        // Esta e a armadilha que o `juntar` e o `unir` ja pagaram, e por isso
        // ela e conferida em vez de suposta: o portao le `"tabela"`, e a
        // pergunta obrigatoria e quem escreve numa tabela sem dizer o nome
        // dela nesse campo. Das operacoes de ESCRITA, as duas sao
        // `duplicar_tabela` e `copiar_tabela`, que gravam no `"destino"` --
        // entao o destino entra aqui junto. As de leitura ficam de fora
        // porque a reserva de transacao **nao barra leitura**: o isolamento e
        // leitura confirmada e NAO bloqueante, e um leitor nunca ve dado nao
        // confirmado porque ele nem chegou ao disco.
        //
        // As que escrevem sem tabela nenhuma -- `criar_database`,
        // `restaurar_backup` -- nao passam por aqui e nao precisam: quem tem
        // transacao aberta ja e recusado por elas em `dentro_da_transacao`, e
        // quem NAO tem esta restaurando um database inteiro, que e uma
        // operacao de administrador com o servico parado.
        if self.transacoes_abertas.load(Ordering::Relaxed) > 0 && OPS_ESCRITA.contains(&op) {
            if let Some(recado) = self.barrado_por_travas(op, pedido, sessao) {
                return Err(PhxError::EmTransacao(recado));
            }
        }
        Ok(())
    }

    fn executar(&self, op: &str, p: &Json, sessao: &Sessao) -> Result<Json> {
        // Pedido 605: a tabela que outra conexao acabou de criar e ainda leva
        // ao disco nao se usa antes de chegar la -- e a espera e AQUI, sem a
        // trava global na mao, para quem espera nao segurar as outras tabelas.
        // Sem nada nascendo, um `load`.
        //
        // # O campo que esta espera le, e quem NAO tem esse campo (pedido 629)
        //
        // Ela lia so `"tabela"`, e o `juntar` com a tabela nova em `b.tabela`
        // passava direto para a espera de DENTRO da trava (`Table::abrir_com`)
        // -- segurando todo escritor do servidor pelo `fsync` de outro. A
        // lista de onde a tabela se esconde ja existe e e UMA:
        // `direito_coluna::tabelas_do_pedido`, a mesma do portao da carga
        // (322). O que ela nao alcanca (a cascata, a mae de uma chave
        // conferida) cai na espera de dentro, que agora tem prazo.
        if phxsql_store::nascendo::quantas() > 0 && !COM_A_TRAVA.with(std::cell::Cell::get) {
            let database = p.texto_ou("database", "").trim();
            let tabelas = crate::direito_coluna::tabelas_do_pedido(op, p);
            if tabelas.is_empty() {
                phxsql_store::catalogo::esperar_pelo_nome(&self.config.base, database, "");
            }
            for t in &tabelas {
                phxsql_store::catalogo::esperar_pelo_nome(&self.config.base, database, t);
            }
        }
        // O PORTAO DAS TRANSACOES, e ele vem antes do despacho inteiro.
        //
        // Um `load(Relaxed)` num `AtomicUsize`, e nada mais, quando nao ha
        // transacao aberta em lugar nenhum -- que e o servidor de hoje. E a
        // licao que o Profiler cobrou: o portao que decide se ha trabalho vem
        // ANTES do trabalho, e nao depois de dois `Json::analisar`.
        if let Some(r) = self.dentro_da_transacao(op, p, sessao) {
            // Erro de classe TRANSACAO derruba a transacao para `ABORT_ONLY`;
            // erro de INSTRUCAO nao mexe nela. E a distincao do §29 do
            // capitulo, e e ela que a aplicacao precisa para saber se corrige
            // e repete ou se desiste e reverte.
            if let Err(e) = &r {
                if crate::transacao::ClasseDoErro::do_erro(e)
                    == crate::transacao::ClasseDoErro::Transacao
                {
                    self.abortar_transacao(sessao.ligacao, &e.to_string());
                }
            }
            return r;
        }
        // O BLOQUEIO DE MANUTENCAO do backup em duas passadas (pedido 513,
        // passo 2): um `load` quando nao ha retrato, e so' entao a lista.
        // A recusa e' `EmMigracao` (4006, `repetir: true`): o backup termina
        // sozinho, e o pedido vale repetido.
        if phxsql_store::congelamento::em_retrato()
            && OPS_DE_MANUTENCAO.contains(&op)
            && phxsql_store::congelamento::em_retrato_de(&self.config.base)
        {
            return Err(phxsql_store::congelamento::recusa_de_manutencao_no_retrato(
                &format!("a operacao {op}"),
            ));
        }
        match op {
            "ping" => Ok(Json::objeto(vec![
                ("phxsql", Json::texto_de(VERSAO)),
                // Para a tela nao cravar numeros que so o servidor sabe
                // (pedido 645): a porta real e os tipos de arquivo que uma
                // tabela pode ter, da lista UNICA do motor de armazenamento.
                ("porta_dados", Json::de_u64(self.porta_dados_agora() as u64)),
                (
                    "arquivos_por_tabela",
                    Json::Lista(
                        phxsql_store::catalogo::Database::extensoes_de_uma_tabela()
                            .iter()
                            .map(|e| Json::texto_de(*e))
                            .collect(),
                    ),
                ),
                // O papel VIVO, das duas fontes que podem muda-lo: o cluster
                // (eleicao e promocao automatica) e a promocao manual do
                // spare. Responder o papel do config.json seria mentir depois
                // de qualquer uma das duas.
                (
                    "papel",
                    Json::texto_de(match &self.cluster {
                        Some(e) => e.papel().nome(),
                        None => self.papel_atual().nome(),
                    }),
                ),
                (
                    "id_servidor",
                    Json::texto_de(&self.config.replicacao.id_servidor),
                ),
                (
                    "conexoes",
                    Json::de_u64(self.permissoes_de_dados.em_uso() as u64),
                ),
                (
                    "no_ar_s",
                    Json::de_u64(((crate::agora_ms() - self.desde_ms) / 1_000).max(0) as u64),
                ),
                (
                    "desde",
                    Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
                ),
            ])),
            "config" => Ok(self.configuracao_json()),
            "config_gravar" => self.op_config_gravar(p, sessao),
            "diretivas" => self.op_diretivas(p, sessao),
            "diretiva_gravar" => self.op_diretiva_gravar(p, sessao),
            "catalogo" => Ok(self.op_catalogo(p, sessao)),
            "sql" => self.op_sql(p, sessao),
            "quem_sou" => Ok(match &sessao.usuario {
                Some(u) => u.ficha(),
                None => Json::objeto(vec![
                    ("usuario", Json::Nulo),
                    ("via", Json::texto_de("token de servico")),
                ]),
            }),
            "usuarios" => Ok(self.cadastro().fichas()),
            "usuario_criar" => self.op_usuario(crate::usuarios::Acao::Criar, p, sessao),
            "usuario_alterar" => self.op_usuario(crate::usuarios::Acao::Alterar, p, sessao),
            "usuario_excluir" => self.op_usuario(crate::usuarios::Acao::Excluir, p, sessao),
            "acessos" => self.op_acessos(p),
            "ips" => self.op_ips(),
            "bloqueios" => self.op_bloqueios(),
            "desbloquear" => self.op_desbloquear(p),
            "bloqueios_exportar" => self.op_bloqueios_exportar(p),
            "whitelist_salvar" => self.op_whitelist_salvar(p),
            "mensagens" => self.op_mensagens(),
            "mensagens_semear" => self.op_mensagens_semear(),
            "idiomas" => self.op_idiomas(p, sessao),
            "idiomas_carga" => self.op_idiomas_carga(p, sessao),
            "idiomas_padrao" => self.op_idiomas_padrao(p, sessao),
            "idiomas_exportar" => self.op_idiomas_exportar(sessao),
            "idiomas_importar" => self.op_idiomas_importar(p, sessao),
            "bancos" => self.op_bancos(),
            "tabelas" => self.op_tabelas(p, sessao),
            "bulkinsert" => self.op_bulkinsert(p, sessao),
            "cargas" => self.op_cargas(),
            // Os tres sinonimos de abertura, porque os tres existem no SQL de
            // todo mundo e quem digita um espera que ele valha.
            "begin" | "start_transaction" | "begin_transaction" => self.op_begin(p, sessao),
            "commit" => self.op_commit(sessao),
            "rollback" => self.op_rollback(sessao),
            "savepoint" => self.op_savepoint(p, sessao),
            "rollback_para" | "rollback_to_savepoint" => self.op_rollback_para(p, sessao),
            "release_savepoint" => self.op_release_savepoint(p, sessao),
            "transacao" => self.op_transacao(sessao),
            "transacoes" => self.op_transacoes(),
            "esquema" => self.op_esquema(p, sessao),
            "servico" => self.op_servico(),
            "servico_parar" => self.op_servico_parar(),
            "servico_subir" => self.op_servico_subir(p),
            "jobs" | "job_listar" => self.op_jobs(p),
            "job_salvar" => self.op_job_salvar(p),
            "job_excluir" => self.op_job_excluir(p),
            "job_rodar" => self.op_job_rodar(p),
            "job_ligar" => self.op_job_ligar(p),
            "criar_database" => self.op_criar_database(p),
            "criar_schema" => self.op_criar_schema(p),
            "criar_tabela" => self.op_criar_tabela(p),
            "acrescentar_coluna" => self.op_acrescentar_coluna(p, sessao),
            "migrar_esquema" => self.op_migrar_esquema(p, sessao),
            "declarar_fk" => self.op_declarar_fk(p, sessao),
            "excluir_fk" => self.op_excluir_fk(p, sessao),
            "redeclarar_indices_texto" => self.op_redeclarar_indices_texto(p, sessao),
            "excluir_tabela" => self.op_excluir_tabela(p),
            "duplicar_tabela" => self.op_duplicar_tabela(p, sessao),
            "copiar_tabela" => self.op_copiar_tabela(p, sessao),
            "renomear_tabela" => self.op_renomear_tabela(p, sessao),
            "sistabelas" | "systables" => self.op_sistabelas(p, sessao),
            "siscolunas" | "syscolumns" => self.op_siscolunas(p, sessao),
            "dados_pessoais" | "lgpd" => self.op_dados_pessoais(p, sessao),
            "sequencias" | "sequences" => self.op_sequencias(p, sessao),
            "ajustar_sequencia" => self.op_ajustar_sequencia(p, sessao),
            "criar_sequencia" => self.op_criar_sequencia(p),
            "proximo_da_sequencia" => self.op_proximo_da_sequencia(p),
            "sequencia" => self.op_sequencia(p),
            "excluir_sequencia" => self.op_excluir_sequencia(p),
            "agrupar" | "group_by" => self.op_agrupar(p, sessao),
            "consultar" => self.op_consultar(p, sessao),
            "criar_visao" => self.op_criar_visao(p, sessao),
            "visoes" => self.op_visoes(p, sessao),
            "excluir_visao" => self.op_excluir_visao(p),
            "pivotar" | "pivot" => self.op_pivotar(p, sessao),
            "juntar" | "join" => self.op_juntar(p, sessao),
            "unir" | "union" => self.op_unir(p, sessao),
            "diferencas" | "diff" => self.op_diferencas(p, sessao),
            "ler" => self.op_ler(p, sessao),
            "varrer" => self.op_varrer(p, sessao),
            "coletar_rowids" => self.op_coletar_rowids(p, sessao),
            "buscar" => self.op_buscar(p, sessao),
            "procurar_texto" => self.op_procurar_texto(p, sessao),
            "inserir" => self.op_inserir(p, sessao),
            "inserir_lote" | "importar" | "carga" => self.op_inserir_lote(p, sessao),
            "importar_conferir" => self.op_importar_conferir(p, sessao),
            "atualizar" => self.op_atualizar(p, sessao),
            "excluir" => self.op_excluir(p, sessao),
            "restaurar" => self.op_restaurar(p, sessao),
            "lixeira" | "trash" => self.op_lixeira(p, sessao),
            "motivos" | "reasons" => self.op_motivos(p, sessao),
            "trilha" | "trilha_lgpd" => self.op_trilha(p, sessao),
            "marcar_lgpd" | "marcar_dado_pessoal" => self.op_marcar_lgpd(p, sessao),
            // Pedido 268: UMA funcao para as duas -- a unica diferenca e o
            // sentido, e duas copias do roteiro (trava, congelamento, FASE A
            // fora da trava) divergiriam na primeira correcao de uma so.
            "criptografar" => self.op_migrar_cifra(p, sessao, true),
            "descriptografar" => self.op_migrar_cifra(p, sessao, false),
            "esvaziar_lixeira" => self.op_esvaziar_lixeira(p, sessao),
            "expurgar_trilha" => self.op_expurgar_trilha(p, sessao),
            "diario" => self.op_diario(p, sessao),
            "profiler_ligar" => self.op_profiler_ligar(p, sessao),
            "profiler_desligar" => self.op_profiler_desligar(sessao),
            "profiler" => self.op_profiler(p, sessao),
            "profiler_limpar" => self.op_profiler_limpar(sessao),
            "posicao" => self.op_posicao(p, sessao),
            "replicar" => self.op_replicar(p, sessao),
            "retrato_da_replica" => self.op_retrato_da_replica(p, sessao),
            "aplicar" => self.op_aplicar(p, sessao),
            "cluster_pulso" => self.op_cluster_pulso(p, sessao),
            "replicar_aguardar" => self.op_replicar_aguardar(p, sessao),
            "cluster_estado" => self.op_cluster_estado(sessao),
            "cluster_no_acrescentar" => self.op_cluster_no_acrescentar(p, sessao),
            "cluster_no_remover" => self.op_cluster_no_remover(p, sessao),
            "replicacao_estado" => self.op_replicacao_estado(),
            "replicacao_testar" => self.op_replicacao_testar(p, sessao),
            "replicacao_ligar" => self.op_replicacao_ligar(p),
            "replicacao_pular" => self.op_replicacao_pular(p),
            // Casca fina: a operacao so escolhe o texto do motivo. Toda a
            // promocao mora em `promover_para_primario`.
            "spare_promover" => {
                self.promover_para_primario(p.texto_ou("motivo", "pedido manual do administrador"))
            }
            "memoria_carregar" => self.op_memoria_carregar(p, sessao),
            "memoria_liberar" => self.op_memoria_liberar(p),
            "memoria" => self.op_memoria(),
            "painel" => self.op_painel(sessao),
            "saude_disco" => Ok(self.op_saude_disco(sessao)),
            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),
            "sessoes" | "processlist" => self.op_sessoes(),
            "telemetria" => self.op_telemetria(p, sessao),
            "telemetria_ligar" => self.op_telemetria_ligar(sessao),
            "telemetria_desligar" => self.op_telemetria_desligar(sessao),
            "telemetria_encerrar" => self.op_telemetria_encerrar(p, sessao),
            "encerrar_sessao" | "kill" => self.op_encerrar_sessao(p),
            "checksum" | "soma_de_verificacao" => self.op_checksum(p, sessao),
            "exportar" | "export" => self.op_exportar(p, sessao),
            "sistema" => Ok(self.op_sistema()),
            "dblink" => self.op_dblink(),
            "dblink_salvar" => self.op_dblink_salvar(p),
            "dblink_excluir" => self.op_dblink_excluir(p),
            "dblink_testar" => self.op_dblink_testar(p),
            "dblink_bancos" => self.op_dblink_bancos(p),
            "dblink_tabelas" => self.op_dblink_tabelas(p),
            "dblink_estrutura" => self.op_dblink_estrutura(p),
            "dblink_ler" => self.op_dblink_ler(p),
            "dblink_consultar" => self.op_dblink_consultar(p),
            "dblink_ligar" => self.op_dblink_ligar(p, sessao),
            "dblink_sincronizar" => self.op_dblink_sincronizar(p, sessao),
            "backup" => self.op_backup(p, sessao),
            "reparar" => self.op_reparar(p, sessao),
            "conferir_backup" => self.op_conferir_backup(p),
            "backups" => self.op_backups(p, sessao),
            "restaurar_backup" => self.op_restaurar_backup(p, sessao),
            // O nome que o Adriano pediu, e o nome em portugues do projeto.
            // Sao a mesma operacao: a interface usa um, o script usa o outro.
            "SelectMemory" | "selectmemory" | "selecionar_memoria" => {
                self.op_selecionar_memoria(p, sessao)
            }
            "verificar" => self.op_verificar(p, sessao),
            "reindexar" => self.op_reindexar(p, sessao),
            outro => Err(PhxError::NaoEncontrado(
                self.msg("erro.operacao_desconhecida", &[("op", outro)]),
            )),
        }
    }
}

/// A recusa de uma trava suja, NOMEANDO a trava -- pedido 458.
///
/// A frase era a mesma em 85 pontos de 14 travas: «uma operacao anterior
/// entrou em panico e deixou a trava suja». Quem lia o `SP000010` nao tinha
/// como saber se era a das rotinas, a do DbLink ou a das transacoes -- e cada
/// uma pede um diagnostico diferente.
fn trava_envenenada(nome: &str) -> PhxError {
    PhxError::Corrompido(format!(
        "a trava \"{nome}\" ficou suja: uma operacao anterior entrou em panico \
         com ela na mao, e o estado atras dela nao se afirma -- a recusa segue \
         ate o servidor reiniciar"
    ))
}

/// A trava de DADOS envenenada sem o reparo ter terminado -- pedido 451.
///
/// Separada da [`trava_envenenada`] porque diz outra coisa: aquela e a
/// recusa das travas comuns, tomadas pelo [`TomarTrava`] (pedido 458), e esta
/// e a da trava de DADOS, com o que ficou pendente no reparo. Com o reparo do `TravaMedida::drop` ela nao
/// deveria sair nunca -- reparo que falha aborta o processo (H5). Ela e o
/// FALHAR FECHADO de qualquer caminho que envenene a trava sem passar por la,
/// e o motivo de ela nao ter virado `unreachable!`: um panico aqui seria o
/// defeito que o pedido existe para fechar.
fn trava_de_dados_sem_reparo(panicos: u64, reparos: u64) -> PhxError {
    PhxError::Corrompido(format!(
        "uma operacao anterior entrou em panico e deixou a trava suja: a trava \
         de dados viu {panicos} panico(s) e {reparos} reparo(s) terminado(s), e \
         nao volta a atender sem o reparo -- reinicie o servidor, o arranque \
         repara"
    ))
}

thread_local! {
    /// Esta thread ja esta com a trava de dados na mao?
    ///
    /// Por thread porque e disso que a reentrancia trata: outra thread pedindo
    /// a trava e o funcionamento normal -- ela espera e recebe. Quem nao pode
    /// esperar e quem ja a tem.
    static COM_A_TRAVA: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// O quorum das esperas deste pedido (pedido 207), que o `despachar` le
    /// e poe na resposta. Por thread pelo molde da `COM_A_TRAVA`: quem espera
    /// e o `Drop` da trava, que nao tem a resposta na mao.
    static QUORUM_DO_PEDIDO: std::cell::RefCell<Option<crate::quorum::Resultado>> =
        const { std::cell::RefCell::new(None) };
}

/// O abraco mortal com a propria trava, transformado em erro.
///
/// Antes disto o servidor simplesmente PARAVA: nem log, nem pilha, nem
/// resposta -- e as outras conexoes paravam junto, porque a trava fica presa
/// na thread pendurada. Um erro nomeado custa um pedido; o abraco custava o
/// servidor.
fn trava_reentrante() -> PhxError {
    PhxError::Corrompido(
        "esta operacao pediu a trava de dados que a propria thread ja tem:          quem chama uma funcao a partir de dentro da trava usa a variante que          recebe a instancia por parametro (`_com`)"
            .into(),
    )
}

/// Uma duracao do pedido, em milissegundos.
///
/// Aceita as duas formas, e as duas de proposito: `"timeout_ms": 5000` para
/// quem monta o JSON por programa, e `"timeout": "5s"` para quem escreve a
/// mao ou traduz do SQL, onde a unidade faz parte do comando. Numero SEM
/// unidade e lido como milissegundo, que e a unidade de todo prazo deste
/// servidor.
fn duracao_ms(p: &Json, campo: &str, padrao: i64) -> Result<i64> {
    let com_ms = format!("{campo}_ms");
    if let Some(n) = p.campo(&com_ms).and_then(Json::inteiro) {
        return Ok(n.max(0));
    }
    let Some(v) = p.campo(campo) else {
        return Ok(padrao);
    };
    if let Some(n) = v.inteiro() {
        return Ok(n.max(0));
    }
    let Some(texto) = v.texto() else {
        return Err(PhxError::Esquema(format!(
            "{campo} precisa ser um numero de milissegundos ou um texto como \"5s\""
        )));
    };
    // O valor pelo `citar` -- parecer SEC do 497, P1. O `TIMEOUT '...'` do
    // SQL chega aqui como texto do pedido montado, e sem teto um megabyte
    // entre aspas somava +1.048.897 B ao `acessos.log` por pedido.
    duracao_de_texto(texto).ok_or_else(|| {
        PhxError::Esquema(format!(
            "{campo}: nao entendi a duracao {}. \
             Aceito 500ms, 5s, 2m ou o numero de milissegundos",
            phxsql_core::error::citar(texto)
        ))
    })
}

/// `"500ms"`, `"5s"`, `"2m"` ou `"1500"` em milissegundos.
fn duracao_de_texto(t: &str) -> Option<i64> {
    let t = t.trim().to_ascii_lowercase();
    // A ordem importa: `ms` tem de ser testado ANTES de `s`, senao "500ms"
    // vira 500 segundos -- que e um erro de mil vezes, calado.
    for (sufixo, fator) in [("ms", 1i64), ("s", 1_000), ("m", 60_000), ("h", 3_600_000)] {
        if let Some(n) = t.strip_suffix(sufixo) {
            return n.trim().parse::<i64>().ok().map(|v| v * fator);
        }
    }
    t.parse::<i64>().ok()
}

/// Esta operacao exige saber QUEM pede? -- pedido 607.
///
/// As dezesseis anonimas (`Atividade::da_operacao` devolve `None`) continuam
/// dezesseis: `begin` sem `SCOPE` nao toma trava nenhuma na abertura, e a
/// primeira escrita dele ja pede login. O `begin` COM `SCOPE` e outra
/// pergunta: ele toma trava de tabela na hora, e a `EXCLUSIVE` barra todo
/// mundo. Antes daqui, so com o token da porta, uma conexao sem login travava
/// `rh.salarios` contra o proprio supervisor. A condicao mora junto do
/// portao do login, e nao dentro do `op_begin`, porque o portao do login e
/// um so -- uma segunda copia de «quem e anonimo» divergiria da primeira.
fn pede_identidade(op: &str, pedido: &Json) -> bool {
    Atividade::da_operacao(op).is_some()
        || (matches!(op, "begin" | "start_transaction" | "begin_transaction")
            && !lista_do_escopo(pedido).is_empty())
}

/// O MESMO erro -- mesmo codigo, mesmo nome, mesmo `repetir` -- com o que
/// quem recebe precisa saber a mais.
///
/// So as familias do PEDIDO e do DADO, que sao as que o `COMMIT` devolve
/// depois de parte gravada (pedido 426): o cliente trata pelo codigo, e o
/// codigo nao pode mudar so porque a frase ganhou um contexto. As outras
/// passam intactas.
fn com_nota(e: PhxError, nota: &str) -> PhxError {
    e.com_nota(nota)
}

/// A resposta de erro do protocolo, com codigo.
///
/// O codigo vem JUNTO com o texto, e nao no lugar dele: o texto e para quem
/// le, o codigo e para quem programa. Sem ele, integrar com o PhxSql obriga a
/// comparar TEXTO -- e melhorar a redacao de uma mensagem quebraria o cliente
/// sem ninguem perceber.
/// Roda a limpeza na saida do escopo, por qualquer caminho.
///
/// Existe por causa dos `return` no meio do laco da conexao: sem ele, cada um
/// deles precisaria lembrar de tirar a conexao do registro, e o dia em que
/// alguem acrescentasse um `return` novo a lista passaria a mostrar conexao
/// que ja morreu -- uma lista que mente e pior do que nenhuma.
/// A trava de dados com o cronometro por dentro.
///
/// Ela se comporta como a `MutexGuard` que embrulha -- `Deref` e `DerefMut`
/// entregam a `Instancia` --, e o que acrescenta acontece no `Drop`: o tempo
/// que ela ficou na mao entra na serie, e -- so no desenrolar de um panico --
/// o REPARO do pedido 451. Sem o primeiro, «o servidor esta lento» nunca
/// distinguiria fila de trabalho; sem o segundo, um panico com ela na mao
/// fechava a base inteira ate alguem reiniciar.
struct TravaMedida<'a> {
    /// O guard de ESCRITA. Vive aqui so para excluir todo mundo enquanto esta
    /// ficha existir -- quem chama nunca o ve. E um CAMPO, e por isso cai
    /// DEPOIS do corpo do `Drop`: o reparo roda com a trava ainda na mao.
    #[allow(dead_code)]
    guarda: std::sync::RwLockWriteGuard<'a, Raiz>,
    /// A ficha exclusiva, que morre junto com o guard acima.
    instancia: Instancia,
    /// Quando a trava foi obtida. `None` com a telemetria desligada -- e ai o
    /// cronometro nao faz nada.
    tomada: Option<Instant>,
    /// O servidor inteiro, e nao so a telemetria: o reparo precisa das
    /// tabelas sujas, das marcas pendentes e dos residentes.
    servidor: &'a Servidor,
    /// A trava foi tomada DURANTE o desenrolar de um panico (pedido 451, M2)?
    ///
    /// E o `AoSair` de uma conexao que caiu por um panico FORA da trava: ele
    /// solta a carga reservada e, para isso, descarrega as sujas -- tomando a
    /// trava com `thread::panicking()` ja verdadeiro. Sem esta marca o `Drop`
    /// acharia que o panico aconteceu com a trava na mao e repararia (ou
    /// abortaria) por um panico que nunca tocou em dado. E a mesma regra do
    /// veneno da `std`: guard tomado no desenrolar nao envenena ao cair.
    tomada_no_desenrolar: bool,
    /// A marca do `COMMIT` desta tomada, do instante antes de ela ir ao disco
    /// ate o destino dela estar decidido -- ver [`MarcaEmVoo`].
    ///
    /// E a UNICA marca que um panico com a trava na mao pode deixar orfa: o
    /// `COMMIT` grava a marca e faz a passada sob esta mesma tomada, e so ele
    /// e a alteracao SOLTA que cascateia (pedido 540, `atualizar_com_a_marca`,
    /// a mesma passada) gravam marca. Morar AQUI, e nao num campo do servidor, e o que faz ela
    /// ainda existir quando o `Drop` roda: um registro com guarda propria,
    /// declarada depois da trava, cairia ANTES dela no desenrolar.
    marca_em_voo: Option<MarcaEmVoo>,
    /// A passagem pelo portao do retrato (pedido 513). O ULTIMO campo de
    /// proposito: campo cai na ordem da declaracao, depois do corpo do
    /// `Drop`, e assim ela so sai da conta quando o guard de ESCRITA ja
    /// caiu. Antes dele, o backup acharia a conta zerada com um escritor
    /// ainda segurando a ficha exclusiva.
    _passagem: crate::retrato::Passagem<'a>,
}

/// A marca do `COMMIT` em voo numa [`TravaMedida`] -- pedido 451.
struct MarcaEmVoo {
    database: String,
    caminho: PathBuf,
    /// O `gravar_marca` ja voltou `Ok`: a marca esta inteira e sincronizada no
    /// disco. Antes disso, marca ausente ou que nao confere e commit que nunca
    /// comecou; depois, marca que nao se rele e FALHA do reparo (M4 da segunda
    /// revisao do DBA), e nao descarte.
    gravada: bool,
}

impl std::ops::Deref for TravaMedida<'_> {
    type Target = Instancia;
    fn deref(&self) -> &Instancia {
        &self.instancia
    }
}

impl std::ops::DerefMut for TravaMedida<'_> {
    fn deref_mut(&mut self) -> &mut Instancia {
        &mut self.instancia
    }
}

/// A trava de dados tomada em modo COMPARTILHADO, com o mesmo cronometro.
///
/// Ela nao entrega a `Instancia`: entrega a [`Raiz`] emprestada, e por ela so
/// se abre tabela de leitura. E a diferenca entre os dois tipos que faz a
/// garantia ser do compilador e nao da disciplina de quem escreve o proximo
/// `op_`.
struct TravaDeLeitura<'a> {
    guarda: std::sync::RwLockReadGuard<'a, Raiz>,
    tomada: Option<Instant>,
    telemetria: &'a crate::telemetria::Telemetria,
}

impl std::ops::Deref for TravaDeLeitura<'_> {
    type Target = Raiz;
    fn deref(&self) -> &Raiz {
        &self.guarda
    }
}

impl Drop for TravaDeLeitura<'_> {
    fn drop(&mut self) {
        COM_A_TRAVA.with(|c| c.set(false));
        if let Some(t) = self.tomada {
            self.telemetria.contar_trava(t.elapsed().as_micros() as u64);
            if let Some(a) = crate::telemetria::corrente() {
                a.sem_a_trava();
            }
        }
    }
}

impl Drop for TravaMedida<'_> {
    fn drop(&mut self) {
        // O PANICO COM A TRAVA NA MAO -- pedido 451, H4 do parecer do DBA.
        //
        // O portao vem ANTES de qualquer trabalho: fora do desenrolar isto e
        // uma leitura de uma variavel da thread, e nada mais. Dentro dele, o
        // reparo roda AQUI, e o lugar e a garantia inteira:
        //
        // * o guard e campo, e so cai depois deste corpo -- ninguem ve o
        //   estado do meio;
        // * o `AoSair` da conexao roda DEPOIS, mais acima na pilha -- as travas
        //   da transacao que caiu (inclusive o fim de tabela que reservava os
        //   rowids) so se soltam quando a marca dela ja foi completada;
        // * e o unico lugar que toma a trava (`so_um_lugar_toma_a_trava`), entao
        //   e o unico lugar onde o conserto precisa estar -- os ~90
        //   `travar_dados()` do servidor nao sabem que ele existe (a H3 que o
        //   parecer reprovou seria a mesma decisao escrita noventa vezes).
        //
        // A marca de reentrancia continua LIGADA durante o reparo de
        // proposito: ele recebe a instancia por parametro, e se alguem um dia
        // pedir a trava de dentro dele, recebe o erro nomeado em vez de
        // esperar por si mesmo.
        //
        // E o panico tem de ter COMECADO com a trava na mao: tomada ja no
        // desenrolar (o `AoSair` de um panico de fora), ela nao viu escrita
        // nenhuma pela metade, e a `std` nem a envenena -- ver
        // `tomada_no_desenrolar`.
        if !self.tomada_no_desenrolar && std::thread::panicking() {
            self.servidor
                .reparar_a_trava(&self.instancia, self.marca_em_voo.as_ref());
        }
        // A ESPERA DO QUORUM -- pedido 207. Aqui, e so aqui, pelo mesmo motivo
        // do reparo acima: e o unico lugar que solta a trava de escrita, e os
        // ~90 `travar_dados()` nao sabem que a espera existe. Com o guard
        // ainda na mao, a linha gravada continua INVISIVEL ate o ok (ou o
        // prazo) -- o `xact.c` do PostgreSQL segura as travas pelo mesmo
        // motivo. O portao e a `Cell` da anotacao, ligada so com o quorum.
        if phxsql_store::log::anotando_tocadas() {
            let tocadas = phxsql_store::log::tomar_tocadas();
            // O retrato do backup (pedido 513, passo 2) soma ANTES de soltar
            // o guard: a fase 2 toma a ficha de leitura so' depois de todo
            // escritor sair, entao ve tudo o que foi anotado aqui.
            phxsql_store::congelamento::anotar_no_retrato(&tocadas);
            if !std::thread::panicking() && !tocadas.is_empty() && self.servidor.quorum_vale_aqui()
            {
                self.servidor.esperar_o_quorum(&self.instancia, tocadas);
            }
        }
        // A unidade do diario fecha junto com a tomada (pedido 676): o evento
        // que esta thread gravar depois, fora da trava, ganha id proprio.
        phxsql_store::log::fechar_unidade();
        // Fora do `if`: a marca de reentrancia nao depende da telemetria, e
        // solta-la so com ela ligada trancaria a thread no modo comum -- que e
        // o modo em que os tres abracos mortais aconteceram.
        COM_A_TRAVA.with(|c| c.set(false));
        if let Some(t) = self.tomada {
            self.servidor
                .telemetria
                .contar_trava(t.elapsed().as_micros() as u64);
            // A atividade deixa de ser a que segura todo mundo no MESMO
            // instante em que solta a trava -- e nao no fim do pedido. Entre
            // um e outro ela ainda monta a resposta, e acusa-la de segurar a
            // trava nesse trecho seria apontar o culpado errado.
            if let Some(a) = crate::telemetria::corrente() {
                a.sem_a_trava();
            }
        }
    }
}

impl Servidor {
    /// A resposta de erro da porta de dados. O texto humano passa pela tabela
    /// de mensagens; `codigo`, `nome`, `classe` e `repetir` NUNCA mudam com o
    /// idioma -- e por eles que o cliente trata.
    /// Os campos de uma resposta de erro, num lugar SO.
    ///
    /// Eram tres copias da mesma lista (protocolo, REST e a resposta com
    /// sessao), e a `sprint` teria de entrar nas tres. E a mesma armadilha do
    /// portao de permissao: a copia que alguem esquecer vira a resposta que
    /// mente, e ninguem acha por leitura. Ha catraca cobrando que continue
    /// sendo uma so (`os_campos_do_erro_saem_de_um_lugar_so`).
    fn campos_do_erro(&self, op: &str, e: &PhxError, ms: u64) -> Vec<(&'static str, Json)> {
        vec![
            ("ok", Json::Bool(false)),
            ("op", Json::texto_de(op)),
            ("erro", Json::texto_de(self.texto_do_erro(e))),
            ("codigo", Json::de_u64(e.codigo() as u64)),
            ("nome", Json::texto_de(e.nome())),
            ("classe", Json::texto_de(e.classe())),
            // A sprint tambem como CAMPO, e nao so na moldura do texto: quem
            // integra le o campo, e ninguem precisa recortar `[SP000008] `
            // de volta da frase. Campo estruturado nao muda com o idioma.
            ("sprint", Json::texto_de(e.sprint())),
            ("repetir", Json::Bool(e.adianta_repetir())),
            ("ms", Json::de_u64(ms)),
        ]
    }

    fn resposta_erro(&self, op: &str, e: &PhxError, ms: u64) -> Json {
        Json::objeto(self.campos_do_erro(op, e, ms))
    }
}

fn posicao_da_coluna(e: &Schema, nome: &str) -> Option<usize> {
    e.colunas().iter().position(|c| c.nome == nome)
}

/// As fontes do servidor: este arquivo e os filhos em `servidor/`, a LISTA
/// UNICA de quem le o servidor como texto (os testes que derivam a lista de
/// operacoes, a catraca da trava, a fase da telemetria).
///
/// Existe porque o `servidor.rs` se divide em arquivos (`docs/propostas/
/// divisao-do-servidor.md`), e cada `include_str!("servidor.rs")` espalhado
/// passaria a ler um pedaco dizendo que leu o todo -- a guarda que conta UMA
/// tomada acharia zero e ninguem perceberia. O texto unido e o mesmo cortado
/// em fronteira de item, entao a regua mede o mesmo numero antes e depois.
///
/// A ordem e `servidor.rs` e depois os filhos pelo caminho em texto, a mesma
/// do `bancada/fontes_do_servidor.py`. O teste `a_lista_das_fontes_bate_com_o_disco`
/// reprova o arquivo novo em `servidor/` que ninguem pos aqui.
#[cfg(test)]
macro_rules! fontes_do_servidor {
    ($($arquivo:literal),* $(,)?) => {
        pub(crate) const FONTES_DO_SERVIDOR: &[(&str, &str)] =
            &[$(($arquivo, include_str!($arquivo))),*];
        /// As fontes da lista unidas num texto so, na mesma ordem.
        pub(crate) const FONTE_DO_SERVIDOR: &str = concat!($(include_str!($arquivo)),*);
    };
}

#[cfg(test)]
fontes_do_servidor! {
    "servidor.rs",
    "servidor/servico_admin_01.rs",
    "servidor/servico_avisos_01.rs",
    "servidor/servico_backup_01.rs",
    "servidor/servico_bidirecional_01.rs",
    "servidor/servico_cluster_01.rs",
    "servidor/servico_composicao_01.rs",
    "servidor/servico_config_01.rs",
    "servidor/servico_consulta_01.rs",
    "servidor/servico_dblink_01.rs",
    "servidor/servico_diario_01.rs",
    "servidor/servico_escrita_01.rs",
    "servidor/servico_esquema_01.rs",
    "servidor/servico_jobs_01.rs",
    "servidor/servico_leitura_01.rs",
    "servidor/servico_marca_01.rs",
    "servidor/servico_mcp_01.rs",
    "servidor/servico_nucleo_01.rs",
    "servidor/servico_permissao_01.rs",
    "servidor/servico_quorum_01.rs",
    "servidor/servico_rede_01.rs",
    "servidor/servico_replicacao_01.rs",
    "servidor/servico_replicacao_02.rs",
    "servidor/servico_sql_01.rs",
    "servidor/servico_telemetria_01.rs",
    "servidor/servico_transacao_01.rs",
    "servidor/servico_web_01.rs",
    "servidor/testes_agregado_na_composicao.rs",
    "servidor/testes_agrupar.rs",
    "servidor/testes_bulkinsert.rs",
    "servidor/testes_cadastro_de_usuarios.rs",
    "servidor/testes_chave_estrangeira.rs",
    "servidor/testes_coletar_rowids.rs",
    "servidor/testes_config_gravar.rs",
    "servidor/testes_conflito.rs",
    "servidor/testes_consultar.rs",
    "servidor/testes_consultar_juncao.rs",
    "servidor/testes_corte_da_composicao.rs",
    "servidor/testes_criar_qualificada.rs",
    "servidor/testes_da_absorcao_do_bidi.rs",
    "servidor/testes_da_cifra_exigida_na_replicacao.rs",
    "servidor/testes_da_ficha_compartilhada.rs",
    "servidor/testes_da_linhagem_na_replica.rs",
    "servidor/testes_da_recusa_por_unicidade.rs",
    "servidor/testes_da_saude_do_disco.rs",
    "servidor/testes_da_sonda_de_rede.rs",
    "servidor/testes_da_varredura_da_fk_fora_da_trava.rs",
    "servidor/testes_das_threads.rs",
    "servidor/testes_database_de_duas_origens.rs",
    "servidor/testes_dblink_cifra.rs",
    "servidor/testes_dblink_dialeto_da_sincronia.rs",
    "servidor/testes_dblink_fora_da_trava.rs",
    "servidor/testes_dblink_valor_no_fio.rs",
    "servidor/testes_diferencas.rs",
    "servidor/testes_direito_por_coluna.rs",
    "servidor/testes_direito_por_tabela.rs",
    "servidor/testes_diretivas.rs",
    "servidor/testes_do_bit_indisponivel_na_trilha.rs",
    "servidor/testes_do_carimbo_do_futuro.rs",
    "servidor/testes_do_lote_de_replicacao.rs",
    "servidor/testes_do_memo_estragado_no_atualizar.rs",
    "servidor/testes_do_panico_sob_a_trava.rs",
    "servidor/testes_do_pular_manual.rs",
    "servidor/testes_do_pulso_que_morre.rs",
    "servidor/testes_do_rebaixar_sem_disco.rs",
    "servidor/testes_do_recuo_no_cluster.rs",
    "servidor/testes_do_relogio_e_do_backup.rs",
    "servidor/testes_do_retrato_do_backup.rs",
    "servidor/testes_do_sal_falso.rs",
    "servidor/testes_do_terceiro_na_tabela_que_nasce.rs",
    "servidor/testes_do_valor_citado_com_teto.rs",
    "servidor/testes_dos_numeros_de_origem.rs",
    "servidor/testes_encerrar_sessao_644.rs",
    "servidor/testes_escala_decimal.rs",
    "servidor/testes_escopo_do_begin_607.rs",
    "servidor/testes_exclusao.rs",
    "servidor/testes_expurgo_da_trilha.rs",
    "servidor/testes_firewall_e_mensagens.rs",
    "servidor/testes_gatilhos.rs",
    "servidor/testes_identidade_uuid.rs",
    "servidor/testes_janela_da_escrita_local.rs",
    "servidor/testes_janela_e_cadeia.rs",
    "servidor/testes_leitura_repetivel.rs",
    "servidor/testes_painel_508.rs",
    "servidor/testes_papel.rs",
    "servidor/testes_pitr.rs",
    "servidor/testes_politica.rs",
    "servidor/testes_portao_do_profiler.rs",
    "servidor/testes_posicao_do_diario.rs",
    "servidor/testes_profiler_desligado.rs",
    "servidor/testes_recusa_sem_dado_pessoal.rs",
    "servidor/testes_regras_de_esquema.rs",
    "servidor/testes_remoto_cifrado.rs",
    "servidor/testes_restaurar_backup.rs",
    "servidor/testes_sequencia_nomeada.rs",
    "servidor/testes_sql.rs",
    "servidor/testes_sql_composto.rs",
    "servidor/testes_sql_dml.rs",
    "servidor/testes_supressao_de_origem.rs",
    "servidor/testes_transacoes.rs",
    "servidor/testes_transacoes/integridade_na_transacao.rs",
    "servidor/testes_transacoes/pedido_514.rs",
    "servidor/testes_transacoes/pre_conferencia_448.rs",
    "servidor/testes_transacoes/revisao_do_dba_448.rs",
    "servidor/testes_trava_suja.rs",
    "servidor/testes_uniao_por_pedido.rs",
    "servidor/testes_upsert.rs",
    "servidor/testes_varrer_expressao.rs",
    "servidor/testes_varrer_onde.rs",
    "servidor/testes_visoes.rs",
    "servidor/testes_volumes_por_quantidade.rs",
}

#[cfg(test)]
mod testes_das_fontes {
    use super::{FONTES_DO_SERVIDOR, FONTE_DO_SERVIDOR};
    use std::path::Path;

    fn juntar(pasta: &Path, base: &Path, saida: &mut Vec<String>) {
        // Pasta que nao existe e lista vazia: antes da divisao, `servidor/`
        // nao existe, e a lista e so o `servidor.rs`.
        let Ok(entradas) = std::fs::read_dir(pasta) else {
            return;
        };
        for e in entradas.flatten() {
            let c = e.path();
            if c.is_dir() {
                juntar(&c, base, saida);
            } else if c.extension().is_some_and(|x| x == "rs") {
                let rel = c.strip_prefix(base).expect("filho da base");
                saida.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    /// Arquivo em `servidor/` fora da lista e texto que nenhum leitor do
    /// servidor enxerga: a catraca que conta tomadas da trava passaria a
    /// contar menos sem nada ter melhorado.
    #[test]
    fn a_lista_das_fontes_bate_com_o_disco() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut filhos = Vec::new();
        juntar(&src.join("servidor"), &src, &mut filhos);
        filhos.sort();
        let mut no_disco = vec!["servidor.rs".to_string()];
        no_disco.extend(filhos);
        let na_lista: Vec<&str> = FONTES_DO_SERVIDOR.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            na_lista, no_disco,
            "FONTES_DO_SERVIDOR difere do disco: ponha o arquivo novo na \
             chamada `fontes_do_servidor!` do `servidor.rs`, na ordem do caminho"
        );
        let soma: usize = FONTES_DO_SERVIDOR.iter().map(|(_, t)| t.len()).sum();
        assert_eq!(soma, FONTE_DO_SERVIDOR.len(), "o texto unido perdeu fonte");
    }
}

#[cfg(test)]
mod testes_politica;

#[cfg(test)]
mod testes_firewall_e_mensagens;

#[cfg(test)]
mod testes_dblink_cifra;

#[cfg(test)]
mod testes_papel;

#[cfg(test)]
mod testes_database_de_duas_origens;

#[cfg(test)]
mod testes_supressao_de_origem;

#[cfg(test)]
mod testes_criar_qualificada;

#[cfg(test)]
mod testes_exclusao;

#[cfg(test)]
mod testes_conflito;

#[cfg(test)]
mod testes_sequencia_nomeada;

#[cfg(test)]
mod testes_direito_por_tabela;

#[cfg(test)]
mod testes_direito_por_coluna;

#[cfg(test)]
mod testes_profiler_desligado;

#[cfg(test)]
mod testes_portao_do_profiler;

#[cfg(test)]
mod testes_bulkinsert;

#[cfg(test)]
mod testes_sql;

#[cfg(test)]
mod testes_gatilhos;

#[cfg(test)]
mod testes_identidade_uuid;

#[cfg(test)]
mod testes_chave_estrangeira;

#[cfg(test)]
mod testes_cadastro_de_usuarios;

#[cfg(test)]
mod testes_config_gravar;

#[cfg(test)]
mod testes_pitr;

#[cfg(test)]
mod testes_restaurar_backup;

#[cfg(test)]
mod testes_janela_e_cadeia;

#[cfg(test)]
mod testes_da_ficha_compartilhada;

#[cfg(test)]
mod testes_transacoes;

#[cfg(test)]
mod testes_varrer_onde;

#[cfg(test)]
mod testes_varrer_expressao;

#[cfg(test)]
mod testes_agrupar;

#[cfg(test)]
mod testes_consultar;

#[cfg(test)]
mod testes_consultar_juncao;

#[cfg(test)]
mod testes_visoes;

#[cfg(test)]
mod testes_upsert;

#[cfg(test)]
mod testes_sql_composto;

#[cfg(test)]
mod testes_diferencas;

#[cfg(test)]
mod testes_posicao_do_diario;

#[cfg(test)]
mod testes_diretivas;

#[cfg(test)]
mod testes_volumes_por_quantidade;

#[cfg(test)]
mod testes_remoto_cifrado;

#[cfg(test)]
mod testes_regras_de_esquema;

#[cfg(test)]
mod testes_sql_dml;

#[cfg(test)]
mod testes_coletar_rowids;

#[cfg(test)]
mod testes_leitura_repetivel;

#[cfg(test)]
mod testes_das_threads;

#[cfg(test)]
mod testes_do_sal_falso;

#[cfg(test)]
mod testes_da_saude_do_disco;

#[cfg(test)]
mod testes_do_lote_de_replicacao;

#[cfg(test)]
mod testes_da_cifra_exigida_na_replicacao;

#[cfg(test)]
mod testes_da_sonda_de_rede;

#[cfg(test)]
mod testes_do_carimbo_do_futuro;

#[cfg(test)]
mod testes_da_recusa_por_unicidade;

#[cfg(test)]
mod testes_dos_numeros_de_origem;

#[cfg(test)]
mod testes_do_pular_manual;

#[cfg(test)]
mod testes_do_bit_indisponivel_na_trilha;

#[cfg(test)]
mod testes_da_varredura_da_fk_fora_da_trava;

#[cfg(test)]
mod testes_do_memo_estragado_no_atualizar;

#[cfg(test)]
mod testes_uniao_por_pedido;

#[cfg(test)]
mod testes_agregado_na_composicao;

#[cfg(test)]
mod testes_escala_decimal;

#[cfg(test)]
mod testes_corte_da_composicao;

#[cfg(test)]
mod testes_expurgo_da_trilha;

#[cfg(all(test, debug_assertions))]
mod testes_do_panico_sob_a_trava;

#[cfg(test)]
mod testes_do_valor_citado_com_teto;

#[cfg(test)]
mod testes_do_pulso_que_morre;

#[cfg(test)]
mod testes_do_relogio_e_do_backup;

#[cfg(test)]
mod testes_recusa_sem_dado_pessoal;

#[cfg(test)]
mod testes_trava_suja;

#[cfg(test)]
mod testes_painel_508;

#[cfg(test)]
mod testes_dblink_fora_da_trava;

#[cfg(test)]
mod testes_dblink_valor_no_fio;

#[cfg(test)]
mod testes_dblink_dialeto_da_sincronia;

#[cfg(test)]
mod testes_do_recuo_no_cluster;

#[cfg(test)]
mod testes_do_rebaixar_sem_disco;

#[cfg(test)]
mod testes_do_retrato_do_backup;

#[cfg(test)]
mod testes_escopo_do_begin_607;

#[cfg(test)]
mod testes_da_linhagem_na_replica;

#[cfg(test)]
mod testes_da_absorcao_do_bidi;

#[cfg(test)]
mod testes_do_terceiro_na_tabela_que_nasce;

#[cfg(test)]
mod testes_janela_da_escrita_local;

#[cfg(test)]
mod testes_encerrar_sessao_644;
