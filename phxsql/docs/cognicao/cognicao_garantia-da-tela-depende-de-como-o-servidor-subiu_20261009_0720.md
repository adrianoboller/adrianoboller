# A garantia escrita na tela depende de como o servidor subiu

**Estado:** PENDENTE

## O que aconteceu

Pedido 454, fatias Z6–Z8. A porta do PhxZipWeb ganhou `--pasta DIR` para a
extração no disco (pelo `phxzip::disco::Destino`, com o dono da pasta
conferido). O rodapé da tela continuou dizendo, em seis idiomas, «O servidor não
lê nem grava nada no seu disco» (`zip.garantia_disco`). Com `--pasta`, isso
passou a ser falso — e só apareceu rodando a tela contra o servidor REAL
(`testes-web/phxzip/real.mjs`), na captura `04-extraido-na-pasta.png`: o
cartão «Extraído na pasta do servidor» e, logo abaixo, a frase dizendo que nada
se grava.

## O que eu concluí primeiro, e estava errado

Que a opção nova era só servidor: um campo a mais no `/api/estado`, um botão
que aparece quando ele é `true`. O laço de chaves fechou (toda chave pedida
existe, nenhuma morta), os 37 testes Rust passaram e o roteiro contra o servidor
falso deu 35/35 — porque o falso não tem `--pasta`, e ali a frase continua
verdadeira. Nenhum conferidor olha se uma frase de GARANTIA ainda vale.

## O que a medição disse

Contra o real, com `--pasta`: a frase errada na tela. Com o texto escolhido
pelo `extrair_na_pasta` do estado (`zip.garantia_disco_com_pasta`), o caso
`estado-e-idioma` do `real.mjs` confere a frase nova, e o `exercitar.mjs`
(falso, sem pasta) continua mostrando a antiga.

E um segundo achado do mesmo roteiro, medido: o `setInputFiles` do Playwright
por CAMINHO descartou calado o arquivo `relatório.txt` (acento no caminho) e
mandou só o `dados.json`; por buffer, o mesmo nome chegou inteiro. O pacote
saiu «certo» com uma entrada a menos, e a lista só acusou na abertura.

## A regra

Quando uma opção muda o que o produto FAZ, procure as frases que prometem o
que ele NÃO faz — elas são as que passam a mentir, e nenhum laço de chaves as
acha.

## Como está guardado hoje

O `real.mjs` confere a frase com `--pasta`. Não há conferidor geral de «frase
de garantia × configuração»: a guarda é este roteiro só.
