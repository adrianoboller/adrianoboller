# Capacidade derivada de um nome que a extensão escolhe herda a concessão de outra

**Estado:** PENDENTE

**Evidência (o que existe, e por que ainda não promove):** `tests/teto_extensao.rs`.
`servidor_de_pacote_com_nome_do_operador_nao_herda_a_concessao` cai com `capacidade_de`
reposto na forma única (`apagar` sai `ok` em vez de `negado`). `plugin_que_se_declara_leitura_ou_mcp_nao_vira_ferramenta`
cai com as duas recusas de `plugins::ferramenta_de` retiradas: no Plan Mode o passo sai `ok` e
escreve no `/work`. Os dois passam com o conserto. Falta o commit onde a prova roda e a
conferência de outra pessoa. Por isso o estado continua PENDENTE.

## O que aconteceu

R7 do radar pedia uma prova: extensão não amplia o teto do operador. A hipótese era que a prova
sairia verde de primeira, porque toda ferramenta de fora passa pelo `call_tool`. Saiu vermelha
em duas portas, e nas duas o portão estava certo. O furo era o **nome** da capacidade que ele
confere:

- Servidor MCP de pacote recebia `mcp.<servidor>`, e quem escolhe `<servidor>` é o pacote. Um
  pacote assinado que declarasse `eco` ao lado do `eco` do operador rodava as ferramentas dele
  sob a concessão que o operador deu ao próprio servidor.
- Plugin assinado declara a própria capacidade primária. Com `fs.read`, que está no padrão e na
  lista de leitura, ele virava ferramenta, entrava no Plan Mode e escrevia no `/work`.

## O que eu concluí primeiro, e estava errado

Que o prefixo `mcp.` já separava as extensões das capacidades nativas, e que isso bastava.
Separava de fato: nenhuma extensão vira `fs.write`. Só que extensão continuava podendo se passar
por **outra extensão**, e um plugin podia se passar por **leitura**. O prefixo isolava o espaço
nativo e deixava aberta a disputa dentro do espaço das extensões.

## A regra

A capacidade que o portão confere é nomeada pela casa, a partir de quem **trouxe** a extensão
(operador, pacote X), e nunca só a partir de um nome que a extensão escolheu. O que a extensão
diz de si (anúncio, manifesto, cabeçalho) pode endurecer a classificação e nunca afrouxá-la.

## Como está guardado hoje

`mcp::capacidade_de` (`mcp.<servidor>` / `mcp.<pacote>.<servidor>`) e a recusa em
`plugins::ferramenta_de` (primária de leitura ou do espaço `mcp.*`). O guia do operador traz a
tabela das portas.
