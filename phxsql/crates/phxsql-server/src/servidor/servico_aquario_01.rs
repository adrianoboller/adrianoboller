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
    pub(super) fn sinalizar_desvio(
        &self,
        aquario: &crate::aquario::Aquario,
        acesso: &Acesso,
        desvio: &crate::aquario::base::Desvio,
    ) {
        if let Some(atividade) = crate::telemetria::corrente() {
            crate::aquario::alarme::sinal_em(&atividade, desvio.alarme, &acesso.op);
        }
        let mut linha = crate::aquario::log::Linha::mudou(
            desvio.alarme,
            None,
            acesso.quando_ms.saturating_add(acesso.duracao_ms as i64),
        );
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
