# O TLS da porta de dados abria o portão e não chegava ao `fio_cifrado`

**Estado:** PENDENTE

## O que aconteceu

Pedido 667 (07/10/2026): para decidir se a senha pode atravessar o fio, fui
perguntar «este fio é cifrado?» e achei **duas respostas** para a mesma
pergunta no `servidor.rs`. O portão da porta de dados lia
`canal.cifrado() || saida.cifrado()` — o TLS do T6a (pedido 572) entrou ali —,
e o `Servidor::fio_cifrado`, que responde a mesma pergunta para o dado pessoal
(LGPD), lia só `sessao.transcricao_do_fio.is_some()`, o túnel Noise. Uma
conexão TLS na porta 5000 passava pelo portão como cifrada e era tratada como
**em claro** na hora de entregar coluna de dado pessoal. Na web, o mesmo:
`fio_cifrado` só respondia «cifrado» com `cifra_fio.exigir` ligado, mesmo
quando o fio era TLS nativo.

## O que eu concluí primeiro, e estava errado

Que bastava escrever o portão da senha com a mesma expressão do portão de
rede (`saida.cifrado()` / `fluxo.cifrado()`). Seria uma terceira resposta para
a mesma pergunta, e a do dado pessoal continuaria torta.

## O que a medição disse

`grep` de `transcricao_do_fio` e `cifrado()` no `servidor.rs`: a `Sessao` não
carregava o TLS em campo nenhum — o `fio_cifrado` não tinha **como** saber.
Com o campo `fio_tls` na sessão (posto onde a sessão nasce: `sessao_do_cabecalho`
nas duas portas HTTP, e na porta de dados ao lado da `saida`), o `fio_cifrado`
passou a ver o TLS, e o portão da senha usa ele. O teste
`senha_em_claro_de_fora_do_loopback_se_recusa_antes_do_cadastro` cai com o TLS
da porta de dados tirado do `fio_cifrado` (RED medido à mão).

## A regra

Quando um meio novo de cifrar entra num portão, procure **quem mais responde
«este fio é cifrado?»** — o irmão é quem faz a mesma pergunta, não quem tem o
mesmo nome.

## Como está guardado hoje

O teste acima cobre a porta de dados por TLS no `op_login`. **Buraco:** não há
teste que entregue coluna de dado pessoal por uma conexão TLS real da porta
5000; a correção do `fio_cifrado` para o LGPD vale por leitura e pelo teste do
login, não por prova própria.
