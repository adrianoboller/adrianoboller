# Número visível só sai de gerador

**Regra.** Todo número que alguém lê (painel, selo de versão, rodapé, tabela de
desempenho, contagem de testes, porcentagem do que falta) sai de um
**gerador**, com a **data em que foi medido** ao lado. Medição que não rodou
aparece como **NÃO MEDIDA**, com o comando para rodar — não some da tabela.

Dois corolários:

- **A receita de um número também envelhece.** Quando o gerador depende de uma
  lista (de arquivos, de páginas, de seções), a lista sai do **código**, nunca
  digitada no gerador.
- **Gerador que faz menos do que o nome promete tem de dizer que fez menos** —
  sob um cabeçalho que não é linha de êxito. Gerador certo chamado pela metade
  entrega número velho anunciando sucesso.

**Cicatrizes.**
- O selo da capa passou **quatro lançamentos** dizendo uma versão que não era.
- Uma lista de três arquivos copiada no gerador fez o rodapé publicar 780 KiB
  quando a interface tinha 1.032 — o código passara a embutir nove.
- Três painéis atrasados **sem um único dígito digitado** (198 pedidos onde
  eram 203, 428 testes onde eram 451): o gerador gravava a página e a contagem,
  imprimia três linhas de êxito e pulava o painel, porque o alvo dele só
  existia se viesse por argumento.
- Juntar resultados de corridas de dias diferentes sem dizer quando publica um
  retrato que nunca existiu. A data sai do resultado, não do `mtime`.
- Um script de edição «sem erro» deixou três estados grudados num título; o
  número da porcentagem é que desmentiu. Depois de escrever, relê-se o estado
  da própria linha.
