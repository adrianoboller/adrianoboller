//! Politica e direito por coluna.
//!
//! Parte da divisao do `servidor.rs` (`docs/propostas/divisao-do-servidor.md`):
//! so movimento; o que outro arquivo do servidor usa passa a `pub(super)`.

use super::*;

/// So em teste: roda no instante exato em que a escrita ja aconteceu e a
/// conta da escrita local ainda nao assentou -- a janela em que a rodada da
/// replica, noutra thread, via o diario andado sem a causa (pedido 630). Sem
/// o gancho a janela e de microssegundos e o defeito so aparece a cada
/// centenas de corridas; com ele, e deterministico.
#[cfg(test)]
type GanchoAposEscrita = std::cell::RefCell<Option<Box<dyn Fn(&Servidor)>>>;
#[cfg(test)]
thread_local! {
    pub(super) static GANCHO_APOS_ESCRITA: GanchoAposEscrita = const { std::cell::RefCell::new(None) };
}

impl Servidor {
    /// A politica, que no `despachar` roda antes de tudo -- para um pedido que
    /// NAO veio pela rede.
    ///
    /// O que fica de fora e o que so faz sentido com um IP do outro lado: a
    /// politica de comando proibido bloqueia quem pediu, e bloquear "o
    /// agendador" ou "o tradutor de SQL" nao quer dizer nada. O resto vale
    /// igual, e mora aqui num lugar so pela razao de sempre: a copia que
    /// alguem esquecer de atualizar vira o furo.
    pub(super) fn politica_do_pedido(&self, op: &str, pedido: &Json) -> Result<()> {
        if self.config.politica.comando_proibido(op) {
            return Err(PhxError::Autorizacao(format!(
                "operacao {op} esta proibida neste servidor pela politica"
            )));
        }
        let base = pedido.texto_ou("database", "");
        if self.config.politica.base_proibida(base) {
            return Err(PhxError::Autorizacao(format!(
                "a base {base} esta proibida neste servidor pela politica"
            )));
        }
        // Mesma sonda de travessia da porta de dados. Um job e escrito por um
        // administrador, mas o arquivo pode ter vindo de outro lugar -- e um
        // `FROM ../../etc` chega pelo tradutor de SQL sem passar pela sonda
        // que o `despachar` fez no pedido de fora.
        for (rotulo, valor) in [
            ("database", base),
            ("tabela", pedido.texto_ou("tabela", "")),
            ("schema", pedido.texto_ou("schema", "")),
        ] {
            if !valor.is_empty() && phxsql_store::catalogo::nome_hostil(valor) {
                return Err(PhxError::Autorizacao(format!(
                    "{rotulo} {valor:?} nao e um nome"
                )));
            }
        }
        Ok(())
    }

    /// Executa um pedido que o SERVIDOR derivou de outro, pelos mesmos portoes.
    ///
    /// # Por que isto existe
    ///
    /// Duas coisas aqui dentro montam um pedido e mandam executar: a op `sql`,
    /// que traduz um `SELECT` para `varrer` ou `buscar`, e -- por outro
    /// caminho -- o agendador de jobs. Nenhuma das duas pode virar a porta dos
    /// fundos: quem nao pode ler a folha de pagamento tambem nao pode le-la
    /// escrevendo `SELECT * FROM folha`, e o portao que confere isso e o
    /// MESMO, lendo o campo `tabela` do pedido TRADUZIDO.
    ///
    /// A licao ja estava escrita no projeto: `juntar` e `unir` foram esse furo
    /// uma vez, porque a tabela delas nao passava pelo campo que o portao olha.
    /// A tradução resolve isso pelo outro lado -- ela PRODUZ o campo que o
    /// portao ja sabe olhar, em vez de pedir um portao novo.
    pub(super) fn executar_derivado(
        &self,
        op: &str,
        pedido: &Json,
        sessao: &Sessao,
    ) -> Result<Json> {
        self.politica_do_pedido(op, pedido)?;
        self.portoes_do_pedido(op, pedido, sessao)?;
        // O direito por COLUNA entra aqui e nao no `executar`, pelo mesmo
        // motivo de o portao entrar aqui: e este o irmao do `despachar`. Um
        // `UPDATE` pelo SQL nunca passa pelo despachar -- ele e um `buscar`,
        // um `ler` e um `atualizar` derivados --, e sem esta linha o `ler`
        // devolveria a linha SEM a coluna negada e o `atualizar` a gravaria
        // nula. Ver `aplicar_direito_por_coluna`.
        self.executar_e_contar_escrita_local(op, pedido, sessao)
    }

    /// O que vem depois dos portoes, nos TRES irmaos que os chamam -- o
    /// `despachar`, o `executar_derivado` e o job: o direito por coluna
    /// embrulhando o `executar`, e a conta da escrita local na replica fiel
    /// (pedido 630) so quando a operacao respondeu `Ok`.
    ///
    /// Mora aqui, e nao em cada irmao, porque a conta tem de ser UMA: no dia
    /// em que um irmao a esquecesse, a escrita local que entrou por ele (um
    /// `INSERT` pelo SQL, um job) tomaria o lugar do evento do source calada.
    /// O portao vem antes do trabalho: fora das escritas que gravam dado
    /// replicado, uma busca numa lista curta e volta.
    pub(super) fn executar_e_contar_escrita_local(
        &self,
        op: &str,
        pedido: &Json,
        sessao: &Sessao,
    ) -> Result<Json> {
        // A camada de protecao (765/767, P1), antes de tudo: o comando da
        // lista de perigo, sem a sessao liberada pela senha de execucao, nao
        // chega nem a contar como escrita local. Mora AQUI pelo motivo do
        // observador logo abaixo -- os tres irmaos passam por este ponto.
        let liberado = self.protecao_do_pedido(op, pedido, sessao)?;
        // O observador de injecao (495, F3) mora AQUI porque aqui passam os
        // tres irmaos -- a rede, a op `sql` derivada e o job -- e porque so
        // aqui se ve o desfecho dos dois lados. O portao e o `bool`, antes
        // de tudo; a vez abre antes do `executar` porque e la dentro que a
        // analise entrega as classes (`crate::injecao`).
        let vez = self
            .config
            .politica
            .observar_injecao_sql
            .then(crate::injecao::abrir);
        // A conta entra ANTES da escrita e sai se ela falhar (pedido 630,
        // corrigido em 02/10/2026): contar depois deixava uma janela entre
        // a escrita aparecer no diario e o contador subir, e a rodada da
        // replica que caisse nela via o diario andado SEM a causa contada --
        // a recusa nomeava as duas causas em vez da local, e o recado
        // ficava guardado por posicao (`continuidade_guardada`). Falhou ao
        // custo de uma entrada transitoria no mapa, desfeita aqui.
        let contada = if grava_dado_replicado(op) {
            self.anotar_escrita_local(pedido)
        } else {
            None
        };
        let r = self.aplicar_direito_por_coluna(op, pedido, sessao);
        #[cfg(test)]
        GANCHO_APOS_ESCRITA.with(|g| {
            if let Some(f) = g.borrow().as_ref() {
                f(self);
            }
        });
        if r.is_err() {
            if let Some(chave) = contada {
                self.desfazer_escrita_local(&chave);
            }
        }
        // Recolhe com `Ok` E com `Err`: a tautologia que da certo e o
        // ataque, e e justamente ela que um gancho so no erro perderia.
        if let Some(vez) = vez {
            self.acusar_injecao(vez.fechar(), pedido);
        }
        // O perigoso que a sessao liberada deixou passar, e que EXECUTOU, vai
        // a trilha. So com `Ok`: o que falhou nao executou, e o erro ja vai ao
        // `acessos.log` como todo erro.
        if let (Some(escopo), true) = (liberado, r.is_ok()) {
            self.registrar_na_trilha("protecao.executou", &escopo, sessao);
        }
        r
    }

    /// A camada de protecao para um pedido -- ver `crate::protecao`.
    ///
    /// O portao e o `match` da lista: op fora dela volta sem alocar nada, e e
    /// esse o caso de quase todo pedido. So as de reescrita medem a tabela.
    ///
    /// Devolve o escopo quando o comando e perigoso e a sessao esta liberada
    /// -- quem executa o leva a trilha depois, se der certo --, `None` quando
    /// nao e da lista, e o erro quando bloqueia.
    pub(super) fn protecao_do_pedido(
        &self,
        op: &str,
        pedido: &Json,
        sessao: &Sessao,
    ) -> Result<Option<crate::protecao::Escopo>> {
        use crate::protecao::{self as pr, Classe};
        let classe = pr::classe(op, pedido);
        if classe == Classe::Livre || !self.config.protecao.ligada {
            return Ok(None);
        }
        let veredito = match classe {
            Classe::Perigosa(categoria) => pr::da_op_perigosa(op, pedido, categoria),
            Classe::SeGrande => self.medir_para_a_protecao(op, pedido, sessao)?,
            Classe::Livre => return Ok(None),
        };
        self.julgar(veredito, sessao)
    }

    /// O veredito contra a sessao, e o REGISTRO dele -- num lugar so, para os
    /// dois pontos (o pedido e o plano). Bloqueado vira ocorrencia pelo
    /// produtor unico (`ComandoBloqueado`, a bolha vermelha da tarefa no
    /// aquario); liberado devolve o escopo para quem executa registrar.
    fn julgar(
        &self,
        veredito: crate::protecao::Veredito,
        sessao: &Sessao,
    ) -> Result<Option<crate::protecao::Escopo>> {
        use crate::protecao::Veredito;
        let Veredito::ExigeSenhaDeExecucao { escopo } = &veredito else {
            return veredito.em_resultado().map(|_| None);
        };
        let escopo = escopo.clone();
        match veredito.para_a_sessao(sessao.execucao_liberada()) {
            Veredito::Livre => Ok(Some(escopo)),
            bloqueado => {
                // `op`, `database` e `tabela` sao os campos que a camada de
                // ocorrencias mantem ao redigir; o MOTIVO e o proprio alarme.
                let dados = Json::objeto(vec![
                    ("op", Json::texto_de(&escopo.op)),
                    ("database", Json::texto_de(&escopo.database)),
                    ("tabela", Json::texto_de(&escopo.tabela)),
                ]);
                crate::telemetria::sinal(
                    crate::aquario::Alarme::ComandoBloqueado,
                    &dados.escrever(),
                );
                bloqueado.em_resultado().map(|_| None)
            }
        }
    }

    /// Uma linha da camada de protecao na trilha administrativa -- o
    /// `diretivas.log`, o mesmo escritor de toda mudanca de administracao
    /// (`crate::diretivas`): quem, de onde, o que e onde.
    pub(super) fn registrar_na_trilha(
        &self,
        recurso: &str,
        escopo: &crate::protecao::Escopo,
        sessao: &Sessao,
    ) {
        let mut pares = vec![
            ("op", Json::texto_de(&escopo.op)),
            ("tabela", Json::texto_de(&escopo.tabela)),
            ("categoria", Json::texto_de(escopo.categoria.nome())),
        ];
        if let Some((linhas, vivas)) = escopo.linhas {
            pares.push(("linhas", Json::de_u64(linhas)));
            pares.push(("vivas", Json::de_u64(vivas)));
        }
        self.anotar_no_diario(
            sessao,
            &escopo.database,
            recurso,
            Json::Nulo,
            Json::objeto(pares),
            &escopo.descrever(),
        );
    }

    /// A camada de protecao para um PLANO que a F8 achou largo: o `DELETE` e
    /// o `UPDATE` por faixa e a cascata do `ao_alterar`. O tamanho so existe
    /// onde o plano fecha, e por isso este e o segundo ponto da mesma
    /// decisao, e nao uma segunda decisao.
    pub(super) fn protecao_do_plano(
        &self,
        rotulo: &str,
        database: &str,
        tabela: &str,
        (linhas, vivas): (u64, u64),
        sessao: &Sessao,
    ) -> Result<()> {
        if !self.config.protecao.ligada {
            return Ok(());
        }
        let veredito = crate::protecao::do_plano(rotulo, database, tabela, linhas, vivas);
        // O plano liberado vai a trilha AQUI, na decisao, e nao depois do
        // laco: a cascata se aplica la dentro do `alterar`, e o laco da faixa
        // pode parar numa linha -- o que se registra e que a sessao liberada
        // mandou executar este plano.
        if let Some(escopo) = self.julgar(veredito, sessao)? {
            self.registrar_na_trilha("protecao.executou", &escopo, sessao);
        }
        Ok(())
    }

    /// A medida das ops que so sao perigosas em tabela grande: os slots (o
    /// que a reescrita paga) e, no indice de texto, se a lista nova tira um
    /// que existe -- que e o DROP INDEX daqui, perigoso em qualquer tamanho.
    ///
    /// # Pelo `esquema`, e nao abrindo a tabela aqui
    ///
    /// Abrir a tabela com a trava na mao e uma secao critica NOVA que alcanca
    /// `fsync` (a abertura pode recuperar), e a catraca `alcancam-fsync-3` do
    /// mapa da trava a reprovou. O `esquema` ja abre a tabela e ja devolve os
    /// dois numeros: perguntar a ele e reusar a secao que existe.
    ///
    /// A tabela que nao abre volta `Livre`: a propria op vai recusar com o
    /// erro dela, que diz o que falta melhor do que a camada diria.
    fn medir_para_a_protecao(
        &self,
        op: &str,
        pedido: &Json,
        sessao: &Sessao,
    ) -> Result<crate::protecao::Veredito> {
        let alvo = Json::objeto(vec![
            ("database", Json::texto_de(pedido.texto_ou("database", ""))),
            ("tabela", Json::texto_de(pedido.texto_ou("tabela", ""))),
        ]);
        let Ok(esquema) = self.executar("esquema", &alvo, sessao) else {
            return Ok(crate::protecao::Veredito::Livre);
        };
        let nomes = |j: Option<&Json>| -> Vec<String> {
            j.and_then(Json::lista)
                .unwrap_or(&[])
                .iter()
                .map(|it| it.texto_ou("nome", "").trim().to_string())
                .collect()
        };
        let tira_indice = op == "redeclarar_indices_texto" && {
            let novos = nomes(
                pedido
                    .campo("indices_texto")
                    .or_else(|| pedido.campo("indices_de_texto")),
            );
            nomes(esquema.campo("indices_texto"))
                .iter()
                .any(|velho| !novos.contains(velho))
        };
        let slots = esquema.inteiro_ou("slots", 0).max(0) as u64;
        Ok(crate::protecao::da_medida(op, pedido, slots, tira_indice))
    }

    // ------------------------------------------- a senha de execucao (767)

    /// `liberar_execucao`: confere a senha de execucao e libera a SESSAO.
    ///
    /// # O que libera, e por quanto tempo
    ///
    /// Precisao do dono: «uma vez informada, a senha fica na sessao; nao e
    /// necessaria para cada comando». A liberacao guarda a identidade e o IP
    /// (`LiberacaoDeExecucao`) e morre com a sessao -- `sair`, novo `login`,
    /// queda da conexao, a sessao web que vence -- ou no `trancar_execucao`.
    /// Na web o id da sessao GIRA ao liberar (`acertar_sessao`).
    ///
    /// # A senha de login nunca libera
    ///
    /// O cadastro ja recusa a de execucao igual a de login. Mas a de LOGIN
    /// pode mudar depois, pelo `usuario_alterar`, e passar a ser igual -- e
    /// ai a segunda senha deixaria de ser segunda. Por isso a conferencia
    /// tambem pergunta, depois do acerto, se o que veio abre o login: se
    /// abre, recusa e pede para trocar uma das duas.
    ///
    /// # A falha conta, e aparece
    ///
    /// Cada falha soma no bloqueio por tentativas PROPRIO (5 seguidas, 15
    /// min -- `crate::senha_de_execucao`) e vira a ocorrencia
    /// `SenhaDeExecucaoRecusada`. A senha nunca entra em mensagem, trilha ou
    /// ocorrencia: so a identidade e o que aconteceu.
    pub(super) fn op_liberar_execucao(&self, p: &Json, sessao: &mut Sessao) -> Result<Json> {
        if self.ainda_anonima(sessao) {
            return Err(PhxError::Autorizacao(self.msg("erro.faca_login", &[])));
        }
        let quem = sessao.identidade_de_execucao().to_string();
        let senha = p.texto_ou("senha", "");
        if senha.is_empty() {
            return Err(PhxError::Esquema(
                "informe \"senha\" com a senha de execucao".into(),
            ));
        }
        phxsql_core::senha::caber_no_teto(senha)?;
        let agora = crate::agora_ms();
        use crate::senha_de_execucao::Conferencia;
        match self.senhas_de_execucao.conferir(&quem, senha, agora)? {
            Conferencia::Confere if self.e_a_senha_de_login(&quem, senha) => {
                self.senha_de_execucao_recusada(&quem);
                Err(PhxError::Autorizacao(format!(
                    "{quem}: a senha de execucao ficou igual a de login -- e ai ela deixa de \
                     ser uma segunda senha. Troque uma das duas (senha_execucao_definir)"
                )))
            }
            Conferencia::Confere => {
                sessao.execucao_liberada = Some(LiberacaoDeExecucao {
                    login: quem.clone(),
                    ip: sessao.ip.clone(),
                });
                self.registrar_na_trilha(
                    "protecao.liberou",
                    &escopo_da_senha("liberar_execucao", &quem),
                    sessao,
                );
                Ok(Json::objeto(vec![
                    ("liberada", Json::Bool(true)),
                    ("login", Json::texto_de(&quem)),
                    (
                        "aviso",
                        Json::texto_de(
                            "vale ate a sessao acabar ou ate o trancar_execucao, e so deste IP",
                        ),
                    ),
                ]))
            }
            outra => {
                self.senha_de_execucao_recusada(&quem);
                Err(recusa_da_senha(&quem, &outra))
            }
        }
    }

    /// `senha_execucao_definir`: cadastra ou troca a senha de execucao.
    ///
    /// # Quem pode, e por que (decisao da P14)
    ///
    /// * **o primeiro cadastro, pelo proprio, com a senha de LOGIN.** Sem
    ///   isso o produto trava: ninguem libera nada. A sessao sozinha nao
    ///   basta -- o id de uma sessao roubada cadastraria a segunda senha do
    ///   dono --, entao a senha de login prova que e ele. No servidor sem
    ///   cadastro, a identidade do servico ja provou o token no portao 1.
    /// * **a troca, pelo proprio, com a senha de execucao ATUAL**, e NAO com a
    ///   de login. A segunda senha existe para o dia em que a de login vazou;
    ///   se a de login bastasse para troca-la, as duas cairiam juntas. A falha
    ///   conta no bloqueio, como no liberar.
    /// * **a redefinicao de OUTRO usuario, pelo administrador com a propria
    ///   sessao liberada.** E a saida de quem esqueceu a dele. Pedir a sessao
    ///   liberada e pedir que o administrador tenha provado a SEGUNDA senha
    ///   dele: a sessao roubada de um administrador nao redefine a de
    ///   ninguem.
    ///
    /// Em todos os casos a nova tem 8 bytes ou mais e e recusada se abrir o
    /// login do dono -- conferida contra o hash da senha de login, sem a
    /// senha de login precisar viajar.
    pub(super) fn op_senha_execucao_definir(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        use crate::senha_de_execucao::{Conferencia, TAMANHO_MINIMO};
        if self.ainda_anonima(sessao) {
            return Err(PhxError::Autorizacao(self.msg("erro.faca_login", &[])));
        }
        let eu = sessao.identidade_de_execucao().to_string();
        let alvo = match p.texto_ou("login", "").trim() {
            "" => eu.clone(),
            outro => outro.to_string(),
        };
        let nova = p.texto_ou("nova_senha_execucao", "");
        phxsql_core::senha::caber_no_teto(nova)?;
        if nova.len() < TAMANHO_MINIMO {
            return Err(PhxError::Esquema(format!(
                "a senha de execucao nova tem de ter {TAMANHO_MINIMO} bytes ou mais \
                 (\"nova_senha_execucao\")"
            )));
        }
        let agora = crate::agora_ms();
        if alvo == eu {
            if self.senhas_de_execucao.tem(&eu)? {
                let atual = p.texto_ou("senha_execucao", "");
                match self.senhas_de_execucao.conferir(&eu, atual, agora)? {
                    Conferencia::Confere => {}
                    outra => {
                        self.senha_de_execucao_recusada(&eu);
                        return Err(recusa_da_senha(&eu, &outra));
                    }
                }
            } else if eu != crate::senha_de_execucao::IDENTIDADE_DO_SERVICO
                && !self.e_a_senha_de_login(&eu, p.texto_ou("senha", ""))
            {
                return Err(PhxError::Autorizacao(format!(
                    "{eu}: o primeiro cadastro da senha de execucao pede a senha de LOGIN \
                     em \"senha\" -- a sessao sozinha nao prova quem voce e"
                )));
            }
        } else {
            let administra = sessao
                .usuario
                .as_ref()
                .is_some_and(|u| u.pode_em("", "", Atividade::Administrar));
            if !administra {
                return Err(PhxError::Autorizacao(format!(
                    "{eu}: so quem administra o servidor redefine a senha de execucao de \
                     outro usuario"
                )));
            }
            if !sessao.execucao_liberada() {
                return Err(PhxError::SenhaDeExecucaoExigida(
                    escopo_da_senha("senha_execucao_definir", &alvo).descrever(),
                ));
            }
            if self.cadastro().por_login(&alvo).is_none() {
                return Err(PhxError::NaoEncontrado(format!(
                    "nao ha usuario com o login {alvo:?}"
                )));
            }
        }
        if self.e_a_senha_de_login(&alvo, nova) {
            return Err(PhxError::Esquema(format!(
                "{alvo}: a senha de execucao nao pode ser a mesma de login -- ela e uma \
                 SEGUNDA senha"
            )));
        }
        self.senhas_de_execucao
            .definir(&alvo, phxsql_core::senha::cifrar(nova), &eu, agora)?;
        self.registrar_na_trilha(
            "protecao.definiu",
            &escopo_da_senha("senha_execucao_definir", &alvo),
            sessao,
        );
        Ok(Json::objeto(vec![
            ("definida", Json::Bool(true)),
            ("login", Json::texto_de(&alvo)),
        ]))
    }

    /// A senha abre o LOGIN desta identidade? Para a do servico, o token.
    /// O que nao se acha (usuario que sumiu) responde que nao.
    fn e_a_senha_de_login(&self, quem: &str, senha: &str) -> bool {
        if senha.is_empty() {
            return false;
        }
        if quem == crate::senha_de_execucao::IDENTIDADE_DO_SERVICO {
            return phxsql_core::hash::iguais_em_tempo_constante(
                senha.as_bytes(),
                self.config.token.as_bytes(),
            );
        }
        let hash = match self.cadastro().por_login(quem) {
            Some(u) => u.senha_hash.clone(),
            None => return false,
        };
        phxsql_core::senha::conferir(senha, &hash)
    }

    /// A ocorrencia da senha de execucao recusada, pelo produtor unico. So a
    /// identidade -- nunca o que foi digitado.
    fn senha_de_execucao_recusada(&self, quem: &str) {
        let dados = Json::objeto(vec![
            ("op", Json::texto_de("liberar_execucao")),
            ("login", Json::texto_de(quem)),
        ]);
        crate::telemetria::sinal(
            crate::aquario::Alarme::SenhaDeExecucaoRecusada,
            &dados.escrever(),
        );
    }

    /// A ocorrencia do pedido que tem a forma de uma injecao. Nao muda a
    /// resposta, nao recusa, nao bloqueia: so registra, para quem
    /// administra. O `dados` e o pedido inteiro, que a camada redige
    /// ANALISANDO -- o texto do SQL normalizado, todo valor `?`.
    fn acusar_injecao(&self, sinais: phxsql_sql::Sinais, pedido: &Json) {
        if sinais.vazio() {
            return;
        }
        crate::telemetria::sinal_com_sinais(
            crate::aquario::Alarme::InjecaoSuspeita,
            &pedido.escrever(),
            sinais,
        );
    }

    /// Tira do mapa a escrita local contada por uma operacao que falhou. A
    /// entrada que chega a zero sai: o teto do mapa continua sendo o numero
    /// de tabelas que existem (revisao SEC, M2), e nao o de nomes tentados.
    fn desfazer_escrita_local(&self, chave: &str) {
        if let Ok(mut m) = self.escritas_locais_na_replica.lock() {
            if let Some(n) = m.get_mut(chave) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    m.remove(chave);
                }
            }
        }
    }

    /// O direito por COLUNA, no unico lugar em que ele existe.
    ///
    /// # Por que ele EMBRULHA o `executar`, em vez de ser um portao
    ///
    /// Porque metade do trabalho e antes e metade e depois. Antes: recusar a
    /// escrita que mudaria a coluna negada, e repor no pedido o valor que
    /// quem nao le a coluna nao tem como mandar. Depois: tirar a coluna da
    /// resposta. Um portao so ve o pedido; uma peneira so ve a resposta.
    /// Separa-los daria dois lugares para esquecer -- e a petrea ja diz o que
    /// acontece com o segundo lugar.
    ///
    /// # Os IRMAOS que chamam isto, e por que sao TRES
    ///
    /// Irmao aqui nao e quem tem nome parecido: e **quem chama
    /// `portoes_do_pedido` e depois `executar`, na mesma ordem**. Sao tres, e
    /// os tres passam por aqui:
    ///
    /// 1. `despachar` -- a rede, e com ela o MCP, o REST e a tela, que entram
    ///    todos pelo `ExecutorLocal`;
    /// 2. `executar_derivado` -- a op `sql` e cada passo que ela produz. E o
    ///    irmao que mais importa: o `UPDATE` pelo SQL e um `buscar` -> `ler`
    ///    -> `atualizar`, e nenhum dos tres chega pelo `despachar`;
    /// 3. `executar_job` -- o agendador, que roda sob o usuario do job.
    ///
    /// Deixar um de fora nao aparece em teste de portao nenhum: o pedido
    /// continua sendo recusado quando a TABELA e negada, e so a coluna vaza.
    ///
    /// # Custo zero para quem nao pediu
    ///
    /// A primeira linha le um `bool` da ficha da sessao. Sem cadastro, sem
    /// usuario, supervisor, ou cadastro sem `"colunas"`: o pedido segue para o
    /// `executar` sem uma alocacao sequer -- nem a busca na tabela de classes.
    /// E a licao do Profiler: o portao que decide se ha trabalho vem ANTES do
    /// trabalho.
    fn aplicar_direito_por_coluna(&self, op: &str, pedido: &Json, sessao: &Sessao) -> Result<Json> {
        let Some(u) = sessao.usuario.as_ref().filter(|u| u.restringe_colunas()) else {
            return self.executar(op, pedido, sessao);
        };
        use crate::direito_coluna::{self as dc, PorColuna};
        let base = pedido.texto_ou("database", "").to_string();
        let tabela = pedido.texto_ou("tabela", "").trim().to_string();

        match dc::classe(op) {
            PorColuna::Nenhum => {
                // `excluir` e `restaurar` nao devolvem coluna nenhuma, e por
                // isso sao `Nenhum` -- mas andam por um rowid escolhido, e
                // «existe»/«nao existe» balde a balde e o mesmo oraculo do
                // pedido 543. O campo que decide e o `rowid` do pedido: quem
                // nao o manda nao paga nem a lista de colunas negadas.
                if pedido.campo("rowid").is_some() {
                    self.recusar_linha_cujo_rowid_revela_coluna_negada(
                        &base,
                        &tabela,
                        &u.colunas_negadas(&base, &tabela, Atividade::Ler),
                        sessao,
                    )?;
                }
                self.executar(op, pedido, sessao)
            }

            // Uma linha por tabela, sem campo `tabela` no pedido: o crivo do
            // oraculo do rowid passa linha a linha (pedido 543).
            PorColuna::Catalogo => {
                let r = self.executar(op, pedido, sessao)?;
                self.peneirar_marca_dagua_por_tabela(r, u, &base, sessao)
            }

            // A marca d'agua de UMA tabela, ao lado do que a operacao fez
            // (pedido 600). A pergunta vem ANTES de executar, e nao depois:
            // o `acrescentar_coluna` e o `migrar_esquema` mudam a tabela, e o
            // esquema que decide e o de quem fez o pedido. Sem `tabela` (a
            // varredura do `migrar_esquema`), linha a linha como o catalogo.
            PorColuna::MarcaDagua => {
                if tabela.is_empty() {
                    let r = self.executar(op, pedido, sessao)?;
                    return self.peneirar_marca_dagua_por_tabela(r, u, &base, sessao);
                }
                let sem_ler = self.negadas_para_ler(u, &base, &tabela, sessao)?;
                // A DEFINICAO que cita coluna negada (revisao SEC, A1 do 245
                // O2b): `calculada: "cpf"` copiava todo CPF para uma coluna
                // que este usuario le, num ALTER so. Recusa na declaracao,
                // antes de tocar em arquivo.
                if op == "acrescentar_coluna" && !sem_ler.is_empty() {
                    let corpo = pedido.campo("coluna").unwrap_or(pedido);
                    if let Ok(c) = crate::valores::coluna_de_json(corpo, 0, false) {
                        if let Some(citada) = dc::definicao_cita_negada(&c, &sem_ler) {
                            return Err(PhxError::Autorizacao(self.msg(
                                "erro.expressao_cita_coluna_negada",
                                &[
                                    ("coluna", &c.nome),
                                    ("citada", &citada),
                                    ("tabela", &format!("{base}.{tabela}")),
                                ],
                            )));
                        }
                    }
                }
                let negada = !sem_ler.is_empty()
                    && self
                        .coluna_do_rowid_negada(&base, &tabela, &sem_ler, sessao)?
                        .is_some();
                let r = self.executar(op, pedido, sessao)?;
                Ok(if negada {
                    dc::peneirar_marca_dagua(r)
                } else {
                    r
                })
            }

            // A definicao de um job e dado que alguem digitou (pedido 350): o
            // valor da coluna negada sai redigido, analisando o pedido.
            PorColuna::PedidoSalvo => Ok(dc::tapar_pedidos_salvos(
                self.executar(op, pedido, sessao)?,
                &|db, t| u.colunas_negadas(db, t, Atividade::Ler),
            )),

            // Devolve (ou grava) dado de linha por um caminho que a peneira
            // nao sabe percorrer. Recusar e mais seguro que vazar.
            PorColuna::Recusa => {
                let alvos = dc::tabelas_do_pedido(op, pedido);
                // Lista vazia nao e "nao toca em tabela": e "nao da para saber
                // qual". `backup` leva os arquivos, `profiler` devolve o texto
                // dos pedidos capturados -- nenhum dos dois nomeia a tabela que
                // alcanca, e adivinhar seria a peneira mentindo.
                let motivo = match alvos.iter().find(|t| u.tem_regra_de_coluna(&base, t)) {
                    Some(t) => Some(format!("ha coluna negada em {base}.{t}")),
                    None if alvos.is_empty() => Some(
                        "esta operacao nao diz que tabela alcanca, e este usuario tem \
                         regra de coluna"
                            .to_string(),
                    ),
                    None => None,
                };
                if let Some(motivo) = motivo {
                    // A SAIDA e por operacao e nao uma frase so: «peca as
                    // colunas por varrer» e conselho para o `juntar` e nao
                    // quer dizer nada para o `agrupar`, que devolve agregado,
                    // nem para o `backup`, que leva arquivo (pedido 245, O5).
                    return Err(PhxError::Autorizacao(format!(
                        "{}: o direito por coluna nao e aplicado em {op}, e {motivo} -- \
                         recusado para nao vazar. {}",
                        u.login,
                        dc::saida(op).texto()
                    )));
                }
                self.executar(op, pedido, sessao)
            }

            // Estrutura NAO e dado: a coluna continua no esquema, e a resposta
            // ganha a lista do que este usuario nao alcanca -- senao a tela
            // pinta um campo que nunca vai chegar preenchido, e quem olha
            // conclui que a coluna esta vazia no banco.
            PorColuna::Estrutura => {
                let r = self.executar(op, pedido, sessao)?;
                let sem_ler = self.negadas_para_ler(u, &base, &tabela, sessao)?;
                let sem_alterar = u.colunas_negadas(&base, &tabela, Atividade::Alterar);
                // O que conta linha por balde e' agregado do DADO, nao da
                // estrutura -- sai quando a coluna que o rowid revela esta em
                // `sem_ler` (pedidos 369 e 543, irmas do 358 pelo outro lado:
                // la o vazamento vinha pelo rowid, aqui pelo balde). Sai
                // antes do atalho de baixo porque ele so olha o par
                // (sem_ler, sem_alterar) vazio, e este e' vazio nos dois
                // quando nao ha regra -- exatamente o caso em que a peneira
                // tambem nao mexe em nada. A resposta ja e o esquema, entao a
                // pergunta «que coluna o rowid revela?» se le dela mesma.
                let r = dc::peneirar_oraculo_do_rowid(r, &sem_ler);
                if sem_ler.is_empty() && sem_alterar.is_empty() {
                    return Ok(r);
                }
                let Json::Objeto(mut pares) = r else {
                    return Ok(r);
                };
                for (nome, lista) in [
                    ("colunas_sem_leitura", sem_ler),
                    ("colunas_sem_alteracao", sem_alterar),
                ] {
                    pares.push((
                        nome.to_string(),
                        Json::Lista(lista.iter().map(Json::texto_de).collect()),
                    ));
                }
                Ok(Json::Objeto(pares))
            }

            PorColuna::Le(onde) => {
                let negadas = self.negadas_para_ler(u, &base, &tabela, sessao)?;
                if negadas.is_empty() {
                    return self.executar(op, pedido, sessao);
                }
                self.recusar_linha_cujo_rowid_revela_coluna_negada(
                    &base, &tabela, &negadas, sessao,
                )?;
                self.recusar_pergunta_sobre_coluna_negada(
                    pedido, sessao, &base, &tabela, &negadas,
                )?;
                Ok(dc::peneirar(
                    self.executar(op, pedido, sessao)?,
                    onde,
                    &negadas,
                ))
            }

            PorColuna::Escreve => {
                if u.regras_de_coluna(&base, &tabela).is_empty() {
                    return self.executar(op, pedido, sessao);
                }
                // A escrita tambem anda por rowid: `atualizar`/`excluir` num
                // rowid escolhido respondem «existe» ou «nao existe», e isso,
                // balde a balde, e o `existe` que a peneira do esquema tira.
                self.recusar_linha_cujo_rowid_revela_coluna_negada(
                    &base,
                    &tabela,
                    &u.colunas_negadas(&base, &tabela, Atividade::Ler),
                    sessao,
                )?;
                let (ajustado, mantidas) =
                    self.escrita_sob_direito_por_coluna(op, pedido, sessao, &base, &tabela)?;
                let r = self.executar(op, &ajustado, sessao)?;
                // A resposta DIZ o que foi mantido contra o pedido. Manter
                // calado seria a resposta errada com cara de certa: o cliente
                // mandou 6000, recebeu `ok`, e o banco tem 5000. O campo so
                // aparece quando ha o que dizer, como `ignorada`/`atualizada`
                // do upsert -- a forma da resposta de quem nao tem regra nao
                // muda.
                if mantidas.is_empty() {
                    return Ok(r);
                }
                let Json::Objeto(mut pares) = r else {
                    return Ok(r);
                };
                pares.push((
                    "colunas_mantidas".to_string(),
                    Json::Lista(mantidas.iter().map(Json::texto_de).collect()),
                ));
                Ok(Json::Objeto(pares))
            }
        }
    }

    /// As colunas que este usuario NAO le nesta tabela: as que o cadastro
    /// nega e as CALCULADAS que citam alguma delas (revisao SEC, achado A1 do
    /// pedido 245 O2b). A calculada `x = cpf` e o CPF por outro nome, e a
    /// peneira que tirasse `cpf` e deixasse `x` peneiraria nada.
    ///
    /// O esquema so se pergunta quando ha coluna negada nesta tabela: quem
    /// nao tem regra nao paga nem a lista. A decisao «esta expressao cita
    /// coluna negada» e a mesma da declaracao (`direito_coluna`).
    fn negadas_para_ler(
        &self,
        u: &crate::usuarios::Usuario,
        base: &str,
        tabela: &str,
        sessao: &Sessao,
    ) -> Result<Vec<String>> {
        let mut negadas = u.colunas_negadas(base, tabela, Atividade::Ler);
        if negadas.is_empty() || tabela.is_empty() {
            return Ok(negadas);
        }
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(base)),
            ("tabela", Json::texto_de(tabela)),
        ]);
        // Tabela que nao existe nao tem calculada: o erro e da operacao, que
        // o devolve com o texto dela logo adiante.
        if let Ok(e) = self.executar("esquema", &ped, sessao) {
            let derivadas = crate::direito_coluna::derivadas_de_negadas(&e, &negadas);
            negadas.extend(derivadas);
        }
        Ok(negadas)
    }

    /// A resposta que traz UMA LINHA POR TABELA na lista `tabelas` (o
    /// `sistabelas`, a varredura do `migrar_esquema`), peneirada linha a
    /// linha -- pedidos 543 e 600. So pergunta o esquema das tabelas em que
    /// ESTE usuario tem coluna negada: as outras saem como sempre sairam, sem
    /// trabalho nenhum.
    ///
    /// O TOTAL do topo tambem sai quando alguma linha saiu: a soma dos
    /// `slots_a_reescrever` com uma tabela pendente so e o numero dela, e
    /// tirar a parcela deixando a soma e tirar nada.
    fn peneirar_marca_dagua_por_tabela(
        &self,
        r: Json,
        u: &crate::usuarios::Usuario,
        base: &str,
        sessao: &Sessao,
    ) -> Result<Json> {
        use crate::direito_coluna as dc;
        let Json::Objeto(pares) = r else {
            return Ok(r);
        };
        let mut alguma = false;
        let mut saida = Vec::with_capacity(pares.len());
        for (k, v) in pares {
            let v = match (k.as_str(), v) {
                ("tabelas", Json::Lista(linhas)) => {
                    let mut novas = Vec::with_capacity(linhas.len());
                    for linha in linhas {
                        let nome = linha.texto_ou("tabela", "").to_string();
                        let sem_ler = u.colunas_negadas(base, &nome, Atividade::Ler);
                        let negada = !sem_ler.is_empty()
                            && self
                                .coluna_do_rowid_negada(base, &nome, &sem_ler, sessao)?
                                .is_some();
                        alguma |= negada;
                        novas.push(if negada {
                            dc::peneirar_marca_dagua(linha)
                        } else {
                            linha
                        });
                    }
                    Json::Lista(novas)
                }
                (_, v) => v,
            };
            saida.push((k, v));
        }
        let r = Json::Objeto(saida);
        Ok(if alguma {
            dc::peneirar_marca_dagua(r)
        } else {
            r
        })
    }

    /// A coluna negada que o ROWID desta tabela revela, se houver -- pedido
    /// 543. Pelo `executar("esquema")`, como o [`Servidor::colunas_do_indice`]
    /// e pelo mesmo motivo: um segundo caminho para ler esquema seria mais um
    /// lugar que a sobreposicao da transacao e o `schema.tabela` teriam de
    /// aprender. Quem chama ja conferiu que `sem_ler` nao e vazia, entao o
    /// esquema so e pedido para quem tem coluna negada NESTA tabela.
    fn coluna_do_rowid_negada(
        &self,
        base: &str,
        tabela: &str,
        sem_ler: &[String],
        sessao: &Sessao,
    ) -> Result<Option<String>> {
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(base)),
            ("tabela", Json::texto_de(tabela)),
        ]);
        let e = self.executar("esquema", &ped, sessao)?;
        Ok(crate::direito_coluna::coluna_do_rowid_negada(&e, sem_ler))
    }

    /// Recusa a operacao de LINHA numa tabela cujo rowid revela coluna negada
    /// a este usuario -- pedido 543, a recusa do 358 alinhada ao direito.
    ///
    /// # Por que recusar, e nao peneirar
    ///
    /// Na particao por letra (ou por periodo), `rowid = (balde - 1) *
    /// registros_por_arquivo + slot`: o rowid de cada linha E o balde, e o
    /// balde e a primeira letra da coluna (ou o mes e o ano dela). Nao ha
    /// peneira que tire isso da resposta sem tirar o rowid -- e sem rowid a
    /// linha nao se le de volta nem se altera. O 358 ja decidiu o mesmo para a
    /// coluna MARCADA, recusando a combinacao na criacao; a coluna negada por
    /// direito tinha protecao menor que a marcada, e era esse o furo. Recusar
    /// e mais seguro que vazar, e a recusa diz as duas saidas que existem.
    fn recusar_linha_cujo_rowid_revela_coluna_negada(
        &self,
        base: &str,
        tabela: &str,
        sem_ler: &[String],
        sessao: &Sessao,
    ) -> Result<()> {
        if sem_ler.is_empty() || tabela.is_empty() {
            return Ok(());
        }
        let Some(coluna) = self.coluna_do_rowid_negada(base, tabela, sem_ler, sessao)? else {
            return Ok(());
        };
        Err(PhxError::Autorizacao(self.msg(
            "erro.rowid_revela_coluna_negada",
            &[("coluna", &coluna), ("tabela", &format!("{base}.{tabela}"))],
        )))
    }

    /// A leitura tambem PERGUNTA, e a pergunta responde sem mostrar a coluna.
    ///
    /// A peneira tira o valor da resposta -- e chega tarde para tres coisas
    /// que acontecem antes dela:
    ///
    /// * `onde` filtrando pela coluna negada: a CONTAGEM das linhas que casam
    ///   e a resposta, e vinte perguntas dessas dizem o salario sem ele nunca
    ///   ter aparecido;
    /// * `indice` cuja chave inclui a coluna negada: `buscar` por chave exata
    ///   diz quem tem aquele valor, e varrer por ele devolve a ordem;
    /// * `coluna` por NUMERO em vez de nome: aqui nao ha esquema para
    ///   resolver a posicao, e adivinhar e pior que recusar.
    ///
    /// O esquema so e pedido quando o pedido nomeia um indice -- entao a
    /// varredura de sempre, que e o laco quente da tela, nao paga nada.
    fn recusar_pergunta_sobre_coluna_negada(
        &self,
        pedido: &Json,
        sessao: &Sessao,
        base: &str,
        tabela: &str,
        negadas: &[String],
    ) -> Result<()> {
        use crate::direito_coluna::mesmo_nome;
        let recusa = |coluna: &str, por_que: &str| -> PhxError {
            PhxError::Autorizacao(format!(
                "a coluna {coluna:?} de {base}.{tabela} nao pode ser lida por este usuario, \
                 e {por_que} responderia sobre ela sem mostra-la"
            ))
        };
        for campo in ["onde", "ordenar"] {
            for f in pedido.campo(campo).and_then(Json::lista).unwrap_or(&[]) {
                let Some(c) = f.campo("coluna") else { continue };
                match c.texto() {
                    Some(nome) => {
                        if let Some(n) = negadas.iter().find(|n| mesmo_nome(n, nome)) {
                            return Err(recusa(n, campo));
                        }
                    }
                    // Coluna por numero: sem o esquema aqui, a posicao 3 pode
                    // ser qualquer uma. Recusar custa um erro que diz o que
                    // fazer; adivinhar custaria a coluna.
                    None => {
                        return Err(PhxError::Autorizacao(format!(
                            "ha coluna negada em {base}.{tabela}: neste caso o campo \
                             {campo:?} tem de nomear a coluna, e nao a posicao dela"
                        )))
                    }
                }
            }
        }
        // A projecao (`colunas`) do SelectMemory: pedir a coluna negada por
        // nome tem de recusar, e nao voltar uma lista com um buraco.
        for c in pedido.campo("colunas").and_then(Json::lista).unwrap_or(&[]) {
            match c.texto() {
                Some(nome) => {
                    if let Some(n) = negadas.iter().find(|n| mesmo_nome(n, nome)) {
                        return Err(recusa(n, "a projecao"));
                    }
                }
                None => {
                    return Err(PhxError::Autorizacao(format!(
                        "ha coluna negada em {base}.{tabela}: a projecao tem de nomear \
                         as colunas, e nao as posicoes delas"
                    )))
                }
            }
        }
        // A EXPRESSAO pergunta sem nomear campo `coluna` nenhum, e por isso
        // ela escapava dos dois lacos de cima: `"expressao": "salario > 5000"`
        // devolve a CONTAGEM de quem ganha mais que isso, e vinte perguntas
        // dessas dizem o salario sem ele nunca ter aparecido -- exatamente o
        // furo que o `onde` ja fechava, por outra porta. O mesmo vale para o
        // `"tendo"`, que fala da linha agregada.
        //
        // A lista de nomes sai do PROPRIO avaliador, e nao de um casador de
        // texto: casador nao sabe que `salarios` nao e `salario`, nem que
        // `'salario'` entre aspas e um literal e nao uma coluna.
        for (campo, como_se_diz) in [("expressao", "a expressao"), ("tendo", "o \"tendo\"")] {
            let Some(txt) = pedido.campo(campo).and_then(Json::texto) else {
                continue;
            };
            if txt.trim().is_empty() {
                continue;
            }
            // Expressao que nao analisa NAO vira recusa de permissao aqui: ela
            // segue e cai adiante, no lugar que sabe dizer em que coluna esta
            // o erro de sintaxe. Um erro de digitacao respondido com «acesso
            // negado» manda procurar no lugar errado.
            let Ok(e) = phxsql_core::expressao::Expressao::analisar(txt) else {
                continue;
            };
            for nome in e.colunas() {
                // Nome QUALIFICADO (`p.salario`), que a junção do `consultar`
                // usa: quem a regra nomeia e a COLUNA, e o prefixo e o apelido
                // do lado. Comparar o nome inteiro deixaria `p.salario`
                // passar por uma regra escrita sobre `salario`.
                let nu = nome.rsplit('.').next().unwrap_or(nome);
                if let Some(n) = negadas.iter().find(|n| mesmo_nome(n, nu)) {
                    return Err(recusa(n, como_se_diz));
                }
            }
        }
        let indice = pedido.texto_ou("indice", "").trim();
        if indice.is_empty() {
            return Ok(());
        }
        let (chave, filtro) = self.colunas_do_indice(base, tabela, indice, sessao)?;
        for c in chave {
            if let Some(n) = negadas.iter().find(|n| mesmo_nome(n, &c)) {
                return Err(recusa(n, format!("o indice {indice:?}").as_str()));
            }
        }
        // O indice PARCIAL e um oraculo que nao aparece na chave: um indice
        // `porId` com `onde: "salario > 5000"` so tem quem ganha mais que
        // isso, e varrer por ele devolve exatamente essa lista -- sem o
        // salario nunca ter aparecido. `buscar` por id responde «este ganha
        // mais que 5000?» uma linha por vez.
        for c in filtro {
            if let Some(n) = negadas.iter().find(|n| mesmo_nome(n, &c)) {
                return Err(recusa(n, format!("o filtro do indice {indice:?}").as_str()));
            }
        }
        Ok(())
    }

    /// As colunas de um indice, lidas do proprio `esquema` do servidor: as
    /// da CHAVE e, separadas, as que o FILTRO (`onde`) do indice parcial cita.
    ///
    /// Pelo `executar` e nao abrindo a tabela na mao: um segundo caminho para
    /// ler esquema seria mais um lugar que a sobreposicao da transacao e a
    /// qualificacao `schema.tabela` teriam de aprender de novo.
    ///
    /// # Por que ela resolve DUAS listas do esquema
    ///
    /// Porque o pedido nomeia o indice num campo so -- `"indice"` --, e o
    /// esquema o devolve em dois: `indices`, as arvores, e `indices_texto`, o
    /// `.fts`. Quem le apenas a primeira nao devolve «nao achei»: devolve
    /// duas listas VAZIAS, e lista vazia atravessa os lacos de quem chama sem
    /// recusar nada. Era assim que `procurar_texto` -- a unica operacao que
    /// nomeia indice de texto -- perguntava pela coluna negada com a
    /// conferencia passando vazia, enquanto o irmao `buscar` recusava pelo
    /// mesmo campo. A petrea diz que o campo que o portao le e o furo; aqui o
    /// furo era o campo ser um e a resposta morar em dois lugares.
    ///
    /// As duas listas vem separadas porque a recusa nomeia qual das duas
    /// respondeu: «o indice» e «o filtro do indice» mandam olhar lugares
    /// diferentes do esquema. O filtro e analisado pelo MESMO avaliador que
    /// o motor usa -- e nao por um casador de texto --, e um filtro que nao
    /// analisa e erro, nao passe livre: ele foi aceito na declaracao, entao
    /// nao analisar aqui e defeito nosso, e defeito nosso fecha a porta.
    fn colunas_do_indice(
        &self,
        base: &str,
        tabela: &str,
        indice: &str,
        sessao: &Sessao,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(base)),
            ("tabela", Json::texto_de(tabela)),
        ]);
        let e = self.executar("esquema", &ped, sessao)?;
        let mut chave = Vec::new();
        let mut filtro = Vec::new();
        for i in e
            .campo("indices")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .filter(|i| i.texto_ou("nome", "") == indice)
        {
            for c in i.campo("colunas").and_then(Json::lista).unwrap_or(&[]) {
                chave.push(c.texto_ou("coluna", "").to_string());
            }
            if let Some(onde) = i.campo("onde").and_then(Json::texto) {
                if !onde.trim().is_empty() {
                    let e = phxsql_core::expressao::Expressao::analisar(onde)?;
                    filtro.extend(e.colunas().iter().map(|c| c.to_string()));
                }
            }
        }
        // O indice de TEXTO entra na mesma lista da chave, e nao numa
        // terceira: a recusa manda olhar o mesmo lugar do esquema -- «o
        // indice tal» --, e um oraculo por palavra responde tanto quanto um
        // por valor exato. A coluna aqui e UMA e vem por NOME, porque e assim
        // que o `esquema` a devolve e assim que o `criar_tabela` a declara.
        for it in e
            .campo("indices_texto")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .filter(|it| it.texto_ou("nome", "") == indice)
        {
            chave.push(it.texto_ou("coluna", "").to_string());
        }
        Ok((chave, filtro))
    }

    /// A escrita numa tabela com regra de coluna: a coluna que este usuario
    /// nao ALTERA fica como esta gravada, venha o que vier no pedido -- e a
    /// funcao devolve o pedido reescrito e a lista do que foi mantido.
    ///
    /// # A regra e UMA: a coluna que nao se altera e mantida, nunca recusada
    ///
    /// Tres estados de uma coluna dentro do pedido -- **valor**, **nulo
    /// explicito** e **ausente** -- e os tres terminam no mesmo lugar: no
    /// `atualizar` (e no upsert que atualiza) a coluna volta com **o valor
    /// gravado**; no `inserir` ela **sai do pedido** e nasce como o motor
    /// decidir, nula ou pelo `padrao`. O ausente ja era assim; o valor e o
    /// nulo eram recusados, e a recusa quebrava toda ficha: a tela manda a
    /// linha INTEIRA, com a coluna que nao le como `null`, e um usuario com
    /// qualquer regra de coluna nao conseguia incluir nem salvar nada -- nem
    /// mexendo so no que podia. «Protecao que quebra todo cliente nao e
    /// protecao, e estrago.» O que motivou tudo isto foi o outro lado da
    /// mesma moeda: o `atualizar` gravando NULL por cima do salario que
    /// ninguem mandou. Manter o gravado resolve os dois de uma vez.
    ///
    /// O que se recusa continua se recusando, mas e OUTRA coisa: ler a
    /// coluna que nao se le (a peneira e a pergunta), e gravar a tabela que
    /// nao se grava (o portao por tabela).
    ///
    /// # O que a resposta diz
    ///
    /// Manter calado seria a resposta errada com cara de certa: o cliente
    /// mandou 6000, recebeu `ok`, e o banco tem 5000. Entao o que foi mantido
    /// CONTRA o pedido volta em `colunas_mantidas`. Para quem LE a coluna,
    /// o valor igual ao gravado nao e mantido contra nada e nao aparece --
    /// e o caso da ficha que devolve a linha inteira como leu. Para quem NAO
    /// le, a comparacao nao acontece e a coluna presente aparece sempre:
    /// «aceito quando bate» seria um oraculo, vinte tentativas e o salario
    /// aparece sem nunca ter sido devolvido.
    ///
    /// # O upsert que ATUALIZA e um `atualizar` para esta funcao
    ///
    /// `inserir` com `se_existir: "atualizar"` grava a linha que ja existe
    /// INTEIRA, pelo mesmo `Table::atualizar` -- e o ausente virava NULL por
    /// cima do valor que este usuario nao pode alterar. Era o defeito que
    /// motivou tudo isto, de volta pela porta do `se_existir`, e ele passou
    /// por todos os testes do `atualizar` porque o `op` aqui dizia
    /// `inserir`. A linha a repor se acha pelo MESMO indice que o upsert vai
    /// usar (`upsert::escolher_indice`), e nao por uma copia da regra; sem
    /// chave completa, ou sem linha com ela, o upsert INSERE e nao ha o que
    /// repor. `ignorar` nao grava nada e fica de fora.
    ///
    /// E o `atualizar` do pedido -- o SET do `ON CONFLICT DO UPDATE` -- e
    /// escrita como qualquer outra: a coluna que este usuario nao altera nao
    /// entra por ele.
    fn escrita_sob_direito_por_coluna(
        &self,
        op: &str,
        pedido: &Json,
        sessao: &Sessao,
        base: &str,
        tabela: &str,
    ) -> Result<(Json, Vec<String>)> {
        use crate::direito_coluna::mesmo_nome;
        let regras: crate::usuarios::RegrasDeColuna = match &sessao.usuario {
            Some(u) => u.regras_de_coluna(base, tabela).to_vec(),
            None => return Ok((pedido.clone(), Vec::new())),
        };

        // A carga COLADA nao passa por aqui, e a recusa e a mesma decisao do
        // Profiler: o que nao se ANALISA nao se redige. Achar o nome de uma
        // coluna recortando um CSV depende de o texto estar escrito de um
        // jeito, e o dia em que nao estiver a coluna negada entra gravada.
        if pedido
            .campo("texto")
            .and_then(Json::texto)
            .is_some_and(|t| !t.trim().is_empty())
        {
            return Err(PhxError::Autorizacao(format!(
                "ha coluna negada em {base}.{tabela}: a carga colada nao e analisada pelo \
                 direito por coluna, e por isso e recusada. Mande as linhas em \"linhas\""
            )));
        }

        // Onde as linhas do pedido moram. `inserir` e `atualizar` mandam uma;
        // `inserir_lote` manda muitas. O nome do campo importa porque e ele
        // que precisa voltar reescrito.
        let campo = ["valores", "linha", "linhas"]
            .into_iter()
            .find(|c| pedido.campo(c).is_some());
        let Some(campo) = campo else {
            // Sem linha nenhuma nao ha o que conferir -- e a operacao recusa
            // por conta, com a mensagem dela.
            return Ok((pedido.clone(), Vec::new()));
        };
        let bruto = pedido.campo(campo).cloned().unwrap_or(Json::Nulo);
        let uma_so = campo != "linhas";
        let linhas: Vec<Json> = if uma_so {
            vec![bruto.clone()]
        } else {
            // `"linhas"` que nao e lista e um pedido malformado, e quem diz
            // isso e a operacao. Reescrever para `[]` trocaria a mensagem
            // dela por «zero linhas gravadas», que e uma resposta de sucesso.
            match bruto.lista() {
                Some(l) => l.to_vec(),
                None => return Ok((pedido.clone(), Vec::new())),
            }
        };

        // A ordem das colunas so e pedida quando alguma linha vem como LISTA
        // -- e ai a posicao e a unica forma de saber qual coluna e qual.
        let ordem: Vec<String> = if linhas.iter().any(|l| matches!(l, Json::Lista(_))) {
            self.colunas_da_tabela(base, tabela, sessao)?
        } else {
            Vec::new()
        };

        // O valor GRAVADO, lido uma vez para a linha inteira. So o `atualizar`
        // -- e o upsert que atualiza -- precisa dele, e so quando ha coluna a
        // repor ou a comparar.
        let upsert = op == "inserir"
            && crate::upsert::SeExistir::de_texto(pedido.texto_ou("se_existir", ""))
                .ok()
                .flatten()
                == Some(crate::upsert::SeExistir::Atualizar);
        let precisa_do_gravado =
            (op == "atualizar" || upsert) && regras.iter().any(|(_, d)| !d.ler || !d.alterar);
        // Rowid zero ou ausente: nao ha linha para ler, e a operacao ja recusa
        // por conta com a mensagem dela. Ler aqui primeiro trocaria «informe
        // "rowid"» por um erro sobre uma linha que ninguem pediu.
        let rowid = pedido.inteiro_ou("rowid", 0).max(0) as u64;
        let gravada = if precisa_do_gravado && upsert {
            self.gravada_do_upsert(pedido, &linhas[0], &ordem, base, tabela, sessao)?
        } else if precisa_do_gravado && rowid > 0 {
            let ped = Json::objeto(vec![
                ("database", Json::texto_de(base)),
                ("tabela", Json::texto_de(tabela)),
                ("rowid", Json::de_u64(rowid)),
                ("com_versao", Json::Bool(true)),
            ]);
            // Pelo `executar`, e nao pelo derivado: o portao de permissao ja
            // foi conferido para ESTE pedido, e um segundo portao aqui
            // recusaria por `ler` quem tem `alterar` e nao tem `ler`.
            self.executar("ler", &ped, sessao)?
        } else {
            Json::Nulo
        };
        let valor_gravado = |coluna: &str| -> Option<Json> {
            let linha = gravada.campo("linha")?;
            match linha {
                Json::Objeto(pares) => pares
                    .iter()
                    .find(|(k, _)| mesmo_nome(k, coluna))
                    .map(|(_, v)| v.clone()),
                _ => None,
            }
        };

        // Ha linha GRAVADA a manter? No `atualizar` e no upsert que atualiza,
        // sim; no `inserir` a coluna que nao se altera simplesmente nao entra.
        let sobre_a_gravada = op == "atualizar" || upsert;
        let mut mantidas: Vec<String> = Vec::new();
        let mut manter = |coluna: &str| {
            if !mantidas.iter().any(|m| mesmo_nome(m, coluna)) {
                mantidas.push(coluna.to_string());
            }
        };
        let mut saida: Vec<Json> = Vec::with_capacity(linhas.len());
        for linha in linhas {
            let mut nova = linha.clone();
            for (coluna, direito) in &regras {
                let posicao = || ordem.iter().position(|c| mesmo_nome(c, coluna));
                let atual = match &nova {
                    Json::Objeto(pares) => pares
                        .iter()
                        .find(|(k, _)| mesmo_nome(k, coluna))
                        .map(|(_, v)| v.clone()),
                    Json::Lista(itens) => posicao().and_then(|i| itens.get(i).cloned()),
                    _ => None,
                };
                match atual {
                    // Ausente: o cliente nao disse nada sobre esta coluna. Quem
                    // nao a le nao tinha como manda-la, e quem nao a altera
                    // nao tinha por que -- nos dois casos o gravado volta.
                    None => {
                        if !sobre_a_gravada || (direito.ler && direito.alterar) {
                            continue;
                        }
                        let Some(v) = valor_gravado(coluna) else {
                            continue;
                        };
                        nova = Self::repor_coluna(nova, coluna, v, posicao());
                    }
                    // Valor ou nulo explicito na coluna que este usuario nao
                    // altera: o gravado fica, e o pedido nao muda nada nela.
                    Some(v) => {
                        if direito.alterar {
                            continue;
                        }
                        match valor_gravado(coluna) {
                            Some(g) => {
                                // O igual, para quem LE: a ficha devolvendo a
                                // linha como leu. Nada foi mantido contra o
                                // pedido, e nada ha a dizer.
                                if direito.ler && g.escrever() == v.escrever() {
                                    continue;
                                }
                                nova = Self::repor_coluna(nova, coluna, g, posicao());
                            }
                            // Sem linha gravada -- o `inserir`, ou o upsert
                            // que vai inserir: a coluna sai, e nasce como o
                            // motor decidir (nula, ou pelo `padrao`).
                            None => nova = Self::tirar_coluna(nova, coluna, posicao()),
                        }
                        manter(coluna);
                    }
                }
            }
            saida.push(nova);
        }

        // O `atualizar` do upsert -- o SET do `ON CONFLICT DO UPDATE` -- e o
        // que entra POR CIMA da linha que ja existe, e a mesma regra vale: a
        // coluna que este usuario nao altera sai do SET, e a mescla parte da
        // linha gravada, que fica como esta.
        let set_ajustado: Option<Json> = match pedido.campo("atualizar") {
            Some(Json::Objeto(set)) => {
                let mut pares = set.clone();
                for (coluna, direito) in &regras {
                    if direito.alterar {
                        continue;
                    }
                    let Some(i) = pares.iter().position(|(k, _)| mesmo_nome(k, coluna)) else {
                        continue;
                    };
                    let igual = direito.ler
                        && valor_gravado(coluna)
                            .is_some_and(|g| g.escrever() == pares[i].1.escrever());
                    if igual {
                        continue;
                    }
                    pares.remove(i);
                    manter(coluna);
                }
                Some(Json::Objeto(pares))
            }
            _ => None,
        };

        let Json::Objeto(pares) = pedido else {
            return Ok((pedido.clone(), mantidas));
        };
        let mut novo: Vec<(String, Json)> = pares
            .iter()
            .filter(|(k, _)| k != campo && !(k == "atualizar" && set_ajustado.is_some()))
            .cloned()
            .collect();
        novo.push((
            campo.to_string(),
            if uma_so {
                saida.into_iter().next().unwrap_or(Json::Nulo)
            } else {
                Json::Lista(saida)
            },
        ));
        if let Some(set) = set_ajustado {
            novo.push(("atualizar".to_string(), set));
        }
        // A VERSAO lida junto com a linha fecha a janela entre o `ler` daqui e
        // o `atualizar` la embaixo: sem ela, quem gravasse no meio teria o
        // valor dele reposto pelo antigo -- perda calada de uma coluna, que e
        // o mesmo estrago que esta funcao existe para impedir. Versao zero
        // (tabela sem controle de versao) nao confere nada, byte a byte como
        // antes; e quem ja mandou a sua nao e sobrescrito.
        if precisa_do_gravado && pedido.campo("versao").is_none() {
            let versao = gravada.inteiro_ou("versao", 0).max(0) as u64;
            if versao > 0 {
                novo.push(("versao".to_string(), Json::de_u64(versao)));
            }
        }
        Ok((Json::Objeto(novo), mantidas))
    }

    /// Tira a coluna da linha: some do objeto, e vira nulo na lista -- a
    /// lista e por posicao, e tirar uma posicao deslocaria todas as outras.
    /// Nos dois casos o motor a trata como «ninguem mandou» e aplica o
    /// `padrao` quando ha um.
    fn tirar_coluna(linha: Json, coluna: &str, posicao: Option<usize>) -> Json {
        use crate::direito_coluna::mesmo_nome;
        match linha {
            Json::Objeto(mut pares) => {
                pares.retain(|(k, _)| !mesmo_nome(k, coluna));
                Json::Objeto(pares)
            }
            Json::Lista(mut itens) => {
                if let Some(i) = posicao {
                    if let Some(slot) = itens.get_mut(i) {
                        *slot = Json::Nulo;
                    }
                }
                Json::Lista(itens)
            }
            outra => outra,
        }
    }

    /// A linha que o upsert vai SOBRESCREVER, no envelope `{"linha": ...}`
    /// que o `ler` devolve -- ou `Nulo` quando nao ha uma.
    ///
    /// Pelo MESMO criterio do motor: `upsert::escolher_indice` sobre os
    /// indices do `esquema`, a chave tirada da linha do pedido, e um `buscar`
    /// por ela. Chave incompleta ou nula quer dizer que o upsert vai INSERIR
    /// (nulo nao e igual a nulo, nem para chave unica), e nao ha o que repor.
    /// A escolha que o motor recusaria -- indice ambiguo, nao unico,
    /// inexistente -- nao e recusada aqui: o `op_inserir` recusa em seguida
    /// com a mensagem dele, e aqui ela so quer dizer «nenhuma linha a repor».
    fn gravada_do_upsert(
        &self,
        pedido: &Json,
        linha: &Json,
        ordem: &[String],
        base: &str,
        tabela: &str,
        sessao: &Sessao,
    ) -> Result<Json> {
        use crate::direito_coluna::mesmo_nome;
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(base)),
            ("tabela", Json::texto_de(tabela)),
        ]);
        let e = self.executar("esquema", &ped, sessao)?;
        let indices = e.campo("indices").and_then(Json::lista).unwrap_or(&[]);
        let candidatos: Vec<crate::upsert::Candidato> = indices
            .iter()
            .map(|i| crate::upsert::Candidato {
                nome: i.texto_ou("nome", "").to_string(),
                unico: i.booleano_ou("unico", false),
                primario: i.booleano_ou("primario", false),
            })
            .collect();
        let Ok(indice) =
            crate::upsert::escolher_indice(tabela, &candidatos, pedido.texto_ou("indice", ""))
        else {
            return Ok(Json::Nulo);
        };
        let colunas: Vec<&str> = indices
            .iter()
            .find(|i| i.texto_ou("nome", "") == indice)
            .and_then(|i| i.campo("colunas").and_then(Json::lista))
            .unwrap_or(&[])
            .iter()
            .map(|c| c.texto_ou("coluna", ""))
            .collect();
        let mut chave = Vec::with_capacity(colunas.len());
        for c in colunas {
            let v = match linha {
                Json::Objeto(pares) => pares
                    .iter()
                    .find(|(k, _)| mesmo_nome(k, c))
                    .map(|(_, v)| v.clone()),
                Json::Lista(itens) => ordem
                    .iter()
                    .position(|o| mesmo_nome(o, c))
                    .and_then(|i| itens.get(i).cloned()),
                _ => None,
            };
            match v {
                Some(v) if !v.e_nulo() => chave.push(v),
                _ => return Ok(Json::Nulo),
            }
        }
        if chave.is_empty() {
            return Ok(Json::Nulo);
        }
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(base)),
            ("tabela", Json::texto_de(tabela)),
            ("indice", Json::texto_de(&indice)),
            ("chave", Json::Lista(chave)),
        ]);
        // Pelo `executar`, como o `ler` do `atualizar`: o portao ja passou.
        let r = self.executar("buscar", &ped, sessao)?;
        match r
            .campo("linhas")
            .and_then(Json::lista)
            .and_then(<[Json]>::first)
        {
            Some(primeira) => Ok(Json::objeto(vec![("linha", primeira.clone())])),
            None => Ok(Json::Nulo),
        }
    }

    /// Poe o valor de volta na linha, seja ela objeto ou lista -- NO LUGAR
    /// da coluna quando ela ja esta la: `json_para_linha` le a primeira
    /// ocorrencia do nome, e uma segunda no fim seria a que ninguem le.
    fn repor_coluna(linha: Json, coluna: &str, valor: Json, posicao: Option<usize>) -> Json {
        use crate::direito_coluna::mesmo_nome;
        match linha {
            Json::Objeto(mut pares) => {
                match pares.iter_mut().find(|(k, _)| mesmo_nome(k, coluna)) {
                    Some(par) => par.1 = valor,
                    None => pares.push((coluna.to_string(), valor)),
                }
                Json::Objeto(pares)
            }
            Json::Lista(mut itens) => {
                if let Some(i) = posicao {
                    while itens.len() <= i {
                        itens.push(Json::Nulo);
                    }
                    itens[i] = valor;
                }
                Json::Lista(itens)
            }
            outra => outra,
        }
    }

    /// Os nomes das colunas da tabela, na ordem do esquema.
    fn colunas_da_tabela(&self, base: &str, tabela: &str, sessao: &Sessao) -> Result<Vec<String>> {
        let ped = Json::objeto(vec![
            ("database", Json::texto_de(base)),
            ("tabela", Json::texto_de(tabela)),
        ]);
        Ok(self
            .executar("esquema", &ped, sessao)?
            .campo("colunas")
            .and_then(Json::lista)
            .unwrap_or(&[])
            .iter()
            .map(|c| c.texto_ou("nome", "").to_string())
            .collect())
    }
}

/// O escopo de uma linha de trilha da senha de execucao: a op e de quem.
fn escopo_da_senha(op: &str, quem: &str) -> crate::protecao::Escopo {
    crate::protecao::Escopo {
        op: op.to_string(),
        database: String::new(),
        tabela: quem.to_string(),
        categoria: crate::protecao::Categoria::Acesso,
        linhas: None,
    }
}

/// A recusa da conferencia, sem nada do que foi digitado.
fn recusa_da_senha(quem: &str, c: &crate::senha_de_execucao::Conferencia) -> PhxError {
    use crate::senha_de_execucao::{Conferencia, BLOQUEIO_MS};
    PhxError::Autorizacao(match c {
        Conferencia::NaoCadastrada => format!(
            "{quem}: nao ha senha de execucao cadastrada -- cadastre pela \
             senha_execucao_definir, com a senha de login"
        ),
        Conferencia::Bloqueada { ate_ms } => format!(
            "{quem}: a senha de execucao esta bloqueada por tentativas ate {}",
            phxsql_core::datahora::instante_iso(*ate_ms)
        ),
        Conferencia::NaoConfere { restantes: 0 } => format!(
            "{quem}: a senha de execucao nao confere, e a conta ficou bloqueada por {} min",
            BLOQUEIO_MS / 60_000
        ),
        Conferencia::NaoConfere { restantes } => format!(
            "{quem}: a senha de execucao nao confere; restam {restantes} tentativa(s) antes \
             do bloqueio"
        ),
        Conferencia::Confere => format!("{quem}: a senha de execucao foi recusada"),
    })
}
