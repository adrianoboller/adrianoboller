# Gerador que falha deixa a saída velha em disco, e o conferidor mente

- **Quando:** 2026-09-02, 19:32
- **Onde:** geração do PDF da página «Subir o PhxSql»
- **Custo:** quase entreguei um PDF sem os dois consertos que eu acabara de
  fazer, dizendo que estavam nele

## O que aconteceu

Gerei o PDF, olhei, achei dois defeitos (uma página em branco no fim e margens
brancas em volta do fundo da marca), consertei o script — e o script passou a
**não compilar**, pela crase de novo. O `node` morreu. Mas os PDFs da rodada
anterior continuavam em disco, com o mesmo nome.

O visualizador rodou, abriu os arquivos **antigos**, e imprimiu «· escuro ·
claro» como se tivesse dado certo. Se eu não tivesse lido a saída de erro do
gerador, teria entregue o PDF velho afirmando que os consertos estavam nele.

## O que eu concluí primeiro, e estava errado

Que «o visualizador rodou e não reclamou» era prova de que o gerador tinha
rodado. São dois programas: um falhou, o outro leu o cadáver do anterior.

## O que a medição disse

O `ls -la` mostrava carimbo de hora **19:30** depois de uma execução que eu
acreditava ser das 19:32. O carimbo de hora era a evidência, e eu quase não
olhei.

## A regra

**Quem gera apaga a saída anterior ANTES de tentar gerar.** Falha então
aparece como ausência, e não como número velho vestido de novo. E: nunca
confira o produto de um passo pelo sucesso do passo seguinte.

## Como está guardado hoje

**Não está** — é o terceiro caso desta família nesta base, e o único sem
guarda:

1. `cargo build --release` não recompila os *examples*, e a bancada mediu uma
   rodada inteira de ganhos com o binário de antes;
2. a bateria web exercitava a página anterior quando `ui/` mudava sem
   recompilar — hoje guardado por `conferirBinario()`;
3. este, o gerador de PDF, sem guarda nenhuma.

O conserto barato é `rm -f` da saída no começo de todo gerador. Aplicado à mão
nesta rodada; ainda não é regra que alguém imponha.
