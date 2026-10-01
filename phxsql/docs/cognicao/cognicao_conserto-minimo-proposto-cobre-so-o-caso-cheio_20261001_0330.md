# O conserto mínimo proposto pela revisão cobre o caso cheio — o vazio e o punido em massa ficam de fora

**Estado:** PENDENTE

## O que aconteceu

Dois consertos do inventário SEC de 01/10/2026 (`docs/propostas/sec-restos-01-10-2026.md`)
vinham com a linha de código pronta, e as duas linhas, aplicadas como vieram,
deixavam um buraco que só o teste mostrou:

- **357-1**: «em `anexar`, `if cofre::ligado() && !cab.cifrado() { self.fechar_ativo()? }`».
  `fechar_ativo` devolve `None` sem fazer nada quando o ativo está **vazio**
  (`trilha.rs`, «fechar volume sem registro so criaria arquivo»). E o ativo
  vazio em claro é exatamente o que um rodízio sem cofre deixa: o primeiro
  registro depois do cofre iria para ele, em claro.
- **284**: «recusar `OPS_DE_REPLICACAO`» no portão 2a-bis. O ramo irmão do
  mesmo portão chama `violacao_leve(&sessao.ip, …)` antes de recusar; copiar
  a forma do irmão contaria violação contra o IP do **proxy**, e a lista negra
  barraria todo cliente que chega por ele.

## O que eu concluí primeiro, e estava errado

Que a linha proposta era o conserto e o trabalho era só escrever o teste
adverso que o inventário nomeava. O teste nomeado
(`ligar_o_cofre_fecha_o_volume_da_trilha_em_claro`) grava um registro sem
cofre e um com — e passa com a linha proposta. Ele não exercita o ativo vazio,
então teria saído verde com o buraco no lugar.

## O que a medição disse

O segundo teste (`ligar_o_cofre_refaz_o_ativo_vazio_em_claro`: um registro,
`fechar_volume_da_trilha`, reinício, cofre, registro) cai com a linha
proposta e passa com o apagar-e-renascer do ativo vazio. A guarda
`trilha-ativo-vazio-em-claro` repõe só a metade proposta e derruba só ele
(1/1), com o teste do caso cheio seguindo verde.

## A regra

Conserto proposto por revisão é hipótese: antes de aplicá-lo, pergunte em que
estado a função chamada **não faz nada**, e se esse estado é alcançável no
caminho do defeito.

## Como está guardado hoje

Pelas duas provas em `crates/phxsql-store/tests/cifra-dos-diarios.rs` e pelas
guardas `trilha-em-claro-depois-do-cofre` e `trilha-ativo-vazio-em-claro`. O
caso do proxy está só no comentário do portão 2a-bis (`servidor.rs`); não há
teste que reprove a volta do `violacao_leve` ali — o buraco fica dito.
