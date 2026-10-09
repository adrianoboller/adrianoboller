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
}
