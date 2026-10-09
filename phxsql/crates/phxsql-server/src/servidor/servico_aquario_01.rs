//! As operacoes do aquario (pedido 707): `aquario_log` e `aquario_contagens`.
//!
//! Casca fina: o portao e daqui, o trabalho e do `crate::aquario`.

use super::*;

impl Servidor {
    /// **Pode MONITORAR este servidor?** -- a pergunta que o portao geral nao
    /// consegue fazer sobre o aquario.
    ///
    /// # Por que ele existe, se o `da_operacao` ja pede `Monitorar`
    ///
    /// Porque o portao geral le o campo `"database"` do pedido, e nenhuma
    /// operacao do aquario tem esse campo de verdade: o aquario e o servidor
    /// inteiro. Sem esta conferencia, quem recebeu `monitorar` so na base
    /// `loja` mandaria `"database":"loja"` e leria a linha do tempo de todas
    /// as bases -- o furo do campo que o portao le, um nivel abaixo do
    /// `juntar`. Aqui a pergunta e feita na regra do SERVIDOR (base vazia, que
    /// cai no `"*"` e no nivel), venha o pedido com o campo que vier.
    ///
    /// Quem administra passa, porque [`crate::usuarios::Permissoes::pode`]
    /// responde `monitorar` tambem por `administrar`: o direito novo entra
    /// pedido, e ninguem perde o que via. Sem cadastro de usuarios, quem
    /// entrou pelo token de servico continua podendo, como em toda operacao de
    /// administracao.
    pub(super) fn portao_do_aquario(&self, sessao: &Sessao) -> Result<()> {
        match &sessao.usuario {
            None => Ok(()),
            Some(u) if u.pode_em("", "", Atividade::Monitorar) => Ok(()),
            Some(u) => Err(PhxError::Autorizacao(format!(
                "{} nao tem permissao de monitorar este servidor: o aquario mostra \
                 a atividade de todas as bases, e o direito vale na regra \"*\" \
                 ou no nivel, nao numa base so",
                u.login
            ))),
        }
    }

    pub(super) fn op_aquario_log(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_do_aquario(sessao)?;
        self.telemetria.aquario().consultar_log(p)
    }

    pub(super) fn op_aquario_contagens(&self, p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_do_aquario(sessao)?;
        self.telemetria.aquario().contagens(p)
    }

    /// `aquario_retrato`: as tarefas vivas com a cor e o motivo, e o
    /// sedimento do servidor (A5).
    ///
    /// # Por que uma op do aquario, e nao `telemetria` com `vista`
    ///
    /// O desenho (§6) pedia `telemetria` com `vista: "aquario"`. Mas a
    /// `telemetria` e `Administrar` no `da_operacao`, e o portao geral olha so
    /// o NOME da op: o usuario da TV, que tem so `monitorar`, seria barrado
    /// antes de a `vista` ser lida -- e ensinar o portao geral a ler um campo
    /// do pedido e reabrir o furo do campo que ele le. O motor continua um so:
    /// o retrato e o `Telemetria::retrato_do_aquario`, que pinta pela mesma
    /// `classe_viva` do painel.
    ///
    /// # O que cada um ve
    ///
    /// Quem so monitora (a TV) ve operacao, tabela, cor e tempo -- nunca login
    /// nem IP (decisao do dono, 09/10, LGPD). Quem passaria no portao da
    /// telemetria -- administrador, ou o token de servico sem cadastro -- ja
    /// ve login e IP la, e ve aqui tambem: a pergunta «quem ve o login?" e a
    /// MESMA, e por isso e o mesmo portao que a responde.
    pub(super) fn op_aquario_retrato(&self, _p: &Json, sessao: &Sessao) -> Result<Json> {
        self.portao_do_aquario(sessao)?;
        let completo = self.portao_da_telemetria(sessao).is_ok();
        Ok(self
            .telemetria
            .retrato_do_aquario(crate::agora_ms(), completo))
    }

    /// A classe de um pedido que TERMINOU, para as linhas `estourou` e
    /// `mudou` do `aquario.log` -- pela [`crate::aquario::classificar`], a
    /// mesma do retrato. Devolve tambem os alarmes que a pintaram.
    ///
    /// Os bits vem da atividade amarrada (o pedido corrente desta thread); o
    /// `fora_do_habitual` vem tambem do julgamento, porque o job sem atividade
    /// nao tem onde guardar o bit e o fato aconteceu do mesmo jeito.
    pub(super) fn classe_do_fim(
        &self,
        acesso: &Acesso,
        habitual: crate::aquario::base::Habitual,
    ) -> (crate::aquario::Classe, u32) {
        let mut alarmes = crate::telemetria::corrente()
            .map(|a| a.alarmes())
            .unwrap_or(0);
        if let Some(d) = habitual.desvio() {
            alarmes |= d.alarme.bit();
        }
        // Com o `us`, o servico e o de verdade (sem a fila); sem ele (job,
        // recusa de porta), a duracao inteira e o que ha.
        let servico_ms = match acesso.us {
            0 => acesso.duracao_ms,
            us => us.saturating_sub(acesso.espera_us) / 1_000,
        };
        let fatos = crate::aquario::Fatos::do_fim(alarmes, servico_ms, acesso.duracao_ms, habitual);
        (
            crate::aquario::classificar(&fatos, &self.telemetria.pintura()),
            alarmes,
        )
    }

    /// O desfecho que a contagem do aquario le (A8), calculado por quem tem a
    /// resposta na mao -- e so com a telemetria ligada: o portao vem antes de
    /// olhar a resposta, e desligada isto custa uma carga atomica.
    ///
    /// Um lugar so para os seis caminhos que anotam um pedido executado
    /// (porta de dados, web, REST, MCP, job e backup agendado): a regra de
    /// classificar e do [`crate::aquario::contagem::Desfecho::da_resposta`].
    pub(super) fn desfecho_para_contar(
        &self,
        op: &str,
        resposta: Option<&Json>,
    ) -> crate::aquario::contagem::Desfecho {
        if !self.telemetria.ligada() {
            return Default::default();
        }
        crate::aquario::contagem::Desfecho::da_resposta(op, resposta)
    }

    /// A virada do minuto da contagem, chamada pelo amostrador. A linha do
    /// minuto fechado vai ao `aquario.log` AQUI, pelo escritor da A6; a
    /// recusa do `aquario-horas.jsonl` vai a saude do disco pelo mesmo
    /// `evento_de_disco` do `acessos.log`. Devolve a linha gravada, para quem
    /// testa a virada.
    pub(super) fn virar_a_contagem(&self, agora_ms: i64) -> Option<Json> {
        let aquario = self.telemetria.aquario();
        let virada = aquario.contagem().virar(agora_ms);
        if let Some(PhxError::Io(io)) = &virada.falha {
            self.evento_de_disco(
                crate::saude_do_disco::classificar(io),
                crate::aquario::contagem::ARQUIVO_DE_HORAS,
                "",
                "",
                &io.to_string(),
            );
        } else if let Some(e) = &virada.falha {
            eprintln!("falha ao gravar o aquario-horas.jsonl: {e}");
        }
        if let Some(minuto) = &virada.minuto {
            let linha = crate::aquario::log::Linha::contagem(agora_ms, minuto.clone());
            self.no_aquario_log(aquario.log().gravar(&linha));
        }
        virada.minuto
    }

    /// As linhas `anel` (780): a tarefa que passou do teto do raio e subiu de
    /// anel vai ao `aquario.log` aqui, pelo escritor de sempre. Devolve
    /// quantas gravou, para quem testa.
    pub(super) fn gravar_os_aneis(&self, agora_ms: i64) -> usize {
        let aquario = self.telemetria.aquario();
        let linhas = self.telemetria.aneis_que_subiram(agora_ms);
        for l in &linhas {
            self.no_aquario_log(aquario.log().gravar(l));
        }
        linhas.len()
    }

    /// As linhas `nasceu` (707, A15): a tarefa que passou de um segundo vai
    /// ao `aquario.log` uma vez por pedido, no relogio do amostrador e pelo
    /// escritor de sempre -- como o anel. Devolve quantas gravou.
    pub(super) fn gravar_as_nascidas(&self, agora_ms: i64) -> usize {
        let aquario = self.telemetria.aquario();
        let linhas = self.telemetria.nascidas(agora_ms);
        for l in &linhas {
            self.no_aquario_log(aquario.log().gravar(l));
        }
        linhas.len()
    }

    /// A linha `morta` (707, A15) de quem `telemetria_encerrar` ou
    /// `encerrar_sessao` vai encerrar, tirada ANTES do ato: depois dele a
    /// operacao pode ter acabado. O portao da telemetria vem primeiro e
    /// antes de qualquer trabalho -- desligada, isto custa um `load`.
    ///
    /// A atividade chega por FUNCAO, e nao por valor, pelo mesmo motivo: o
    /// `encerrar_sessao` a acha numa busca no registro, e um argumento seria
    /// avaliado antes do portao -- o trabalho que o portao existe para poupar.
    pub(super) fn retrato_da_morta(
        &self,
        atividade: impl FnOnce() -> Option<Arc<crate::telemetria::Atividade>>,
    ) -> Option<crate::aquario::log::Linha> {
        self.telemetria.aquario_se_ligada()?;
        let a = &atividade()?;
        if a.estado() == crate::telemetria::Estado::Ociosa {
            return None;
        }
        Some(self.telemetria.linha_da_morta(a, crate::agora_ms()))
    }

    /// Grava a `morta` tirada antes do ato, com o desfecho que o ato deu
    /// (`encerrando`, `marcada`, `nao_cancelavel`, `derrubada`). O desfecho
    /// e o MOTIVO da morte, e vai pela palavra do protocolo, nao por frase:
    /// a tela o traduz.
    pub(super) fn gravar_a_morta(&self, linha: Option<crate::aquario::log::Linha>, desfecho: &str) {
        let Some(mut l) = linha else {
            return;
        };
        l.dados = Some(Json::objeto(vec![("desfecho", Json::texto_de(desfecho))]));
        self.no_aquario_log(self.telemetria.aquario().log().gravar(&l));
    }

    /// O arranque no meio da hora: a contagem refaz a hora corrente (e fecha
    /// a anterior que o processo velho nao fechou) das linhas `contagem` que
    /// ficaram no `aquario.log`. Chamado depois de os dois arquivos estarem
    /// definidos -- o `retomar` confere o `aquario-horas.jsonl` para nao
    /// gravar duas vezes a mesma hora.
    ///
    /// Le DUAS horas para tras: a corrente, e a anterior inteira, que pode
    /// ter caido nos ultimos segundos sem fechar.
    pub(super) fn retomar_a_contagem(&self, agora_ms: i64) {
        use crate::aquario::contagem::HORA_MS;
        let aquario = self.telemetria.aquario();
        let desde = agora_ms - agora_ms.rem_euclid(HORA_MS) - HORA_MS;
        match aquario.log().contagens_desde(desde) {
            Ok(linhas) => aquario.contagem().retomar(&linhas, agora_ms),
            // O aquario e acessorio: sem as linhas, a hora corrente recomeca
            // parcial -- `minutos_medidos` diz quantos faltam --, e o servidor
            // sobe do mesmo jeito.
            Err(e) => eprintln!("AVISO: a contagem do aquario nao se retomou do aquario.log: {e}"),
        }
    }

    /// O pedido que a base da A4 achou fora do habitual vira alarme pelo
    /// produtor UNICO da A3 -- o bit na tarefa que esta terminando --, e a
    /// linha `mudou` do `aquario.log` (A6) o registra.
    ///
    /// Chamado so de dentro do portao da telemetria (o `anotar`), e so no
    /// caminho raro: a base devolve `None` para quase todo pedido.
    ///
    /// Sem atividade amarrada (job, backup agendado) nao ha tarefa onde por o
    /// bit, mas o fato aconteceu: a linha `mudou` sai do mesmo jeito, com a
    /// operacao e a tabela, para a TV que abre depois achar o pedido lento.
    ///
    /// A cor da linha sai da [`crate::aquario::classificar`], pela mesma
    /// [`Servidor::classe_do_fim`] do `estourou` (A5).
    pub(super) fn sinalizar_desvio(
        &self,
        aquario: &crate::aquario::Aquario,
        acesso: &Acesso,
        habitual: crate::aquario::base::Habitual,
        desvio: &crate::aquario::base::Desvio,
    ) {
        // A linha `mudou` repete o id da ocorrencia (495, F2, §11.3): o
        // VALOR, nunca a decisao.
        let ocorrencia = crate::telemetria::corrente().and_then(|atividade| {
            crate::aquario::alarme::sinal_em(&atividade, desvio.alarme, &acesso.op)
        });
        let (classe, alarmes) = self.classe_do_fim(acesso, habitual);
        let mut linha = crate::aquario::log::Linha::mudou(
            desvio.alarme,
            ocorrencia,
            acesso.quando_ms.saturating_add(acesso.duracao_ms as i64),
        )
        .com_classe(classe, alarmes);
        linha.op = acesso.op.clone();
        linha.database = acesso.database.clone();
        linha.tabela = acesso.tabela.clone();
        linha.dados = Some(Json::objeto(vec![
            ("z", Json::Numero(desvio.z)),
            ("n", Json::de_u64(desvio.n)),
            ("p95_habitual_us", Json::de_u64(desvio.p95_habitual_us)),
            ("servico_us", Json::de_u64(desvio.servico_us)),
        ]));
        self.no_aquario_log(aquario.log().gravar(&linha));
    }

    /// O destino da falha de gravacao do `aquario.log`, um so para as tres
    /// linhas que o servidor grava (`estourou`, `mudou`, `contagem`): o erro
    /// de E/S e noticia de disco, como o do `acessos.log`, e nunca sobe ao
    /// cliente, que nao pediu linha nenhuma.
    pub(super) fn no_aquario_log(&self, gravou: Result<()>) {
        if let Err(PhxError::Io(io)) = gravou {
            self.evento_de_disco(
                crate::saude_do_disco::classificar(&io),
                crate::aquario::log::NOME_DO_ARQUIVO,
                "",
                "",
                &io.to_string(),
            );
        }
    }
}
