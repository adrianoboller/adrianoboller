# A apresentacao do produto vive em tres superficies, e a limpeza pegou uma

Data da descoberta: 12/09/2026, 01:25.

## 1. O que aconteceu

A rodada anterior (commit `12a28c5`) fechou "HFSQL fora da superficie do
produto". Mas "superficie" ali significou so as maquetes do correio
(`docs/dossie/tela-servermail.html` e `tela-clientmail.html`). A apresentacao
do PROPRIO produto continuava com a etiqueta: a mesma frase "Motor de dados em
Rust no modelo de arquivos separados do HFSQL(R)" vivia em tres superficies
visiveis nao geradas -- o "Sobre" da tela (`crates/phxsql-server/ui/index.html`,
funcao do painel Sobre), a chamada do README (`README.md`) e a chamada do
dossie (`docs/dossie/dossie-phxsql-0.18.html`).

## 2. O que eu conclui primeiro, e estava errado

Lendo o resumo da rodada, conclui que "a superficie do produto ja estava
limpa" e que so faltava decidir o alcance do resto. Estava errado: a limpeza
anterior cobria as maquetes, nao o produto. A etiqueta de apresentacao e UMA
frase copiada a mao em tres telas visiveis, sem gerador que as mantenha em
sincronia -- e limpar uma delas e parar e o mesmo estrago do numero digitado
que envelhece calado, so que na prosa da capa.

## 3. O que a medicao disse

`grep -i hfsql` depois da limpeza do correio: **260 ocorrencias em 60
arquivos**. Da superficie visivel nao gerada, a frase de apresentacao aparecia
**3 vezes**, identicas. As outras ocorrencias visiveis do `index.html` eram
comparativo/atribuicao (a tabela "De onde vem" e o aviso de marcas) -- que o
dono mandou ficar, porque a petrea "bancada compara trabalho igual" pede o
concorrente nomeado. As demais 250+ eram comentario, pesquisa, cognicao,
CHANGELOG e gerados. No dossie, fonte local x publicado batiam (10 = 10 HFSQL
antes da edicao, mesma versao 0.18.0); a diferenca de 348 bytes era so o
esqueleto que o host embrulha na publicacao, nao divergencia de conteudo.

## 4. A regra

Quando o dono mandar tirar uma etiqueta de apresentacao, procure a MESMA frase
nas tres superficies visiveis nao geradas -- o "Sobre" da tela, o README e a
chamada do dossie -- antes de dizer "feito". Limpar uma e meio-servico, e a
que ficou e a que grita mais (o README e a porta da frente).

## 5. Como esta guardado hoje

As tres viraram "modelo de arquivos separados por tabela" (commit `df989e2`),
e o dossie foi republicado (versao 27). O buraco que ficou: **nao ha gerador
que sincronize as tres** -- a frase de apresentacao segue digitada a mao em
tres lugares, e a proxima mudanca vai precisar achar os tres de novo. Um
gerador unico da apresentacao fecharia o buraco, mas nao foi pedido, e a
petrea "guarda nova entra pedida, nao imposta" manda registrar o buraco em vez
de impor a guarda.
