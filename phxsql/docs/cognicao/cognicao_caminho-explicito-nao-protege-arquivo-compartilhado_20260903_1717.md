# Caminho explícito não protege um ARQUIVO que duas frentes editam junto

## 1. O que aconteceu

Trabalhando no pedido 150 (zelador), editei `docs/PENDENCIAS.md` — só a linha
do próprio pedido 150 — e deixei a edição no disco, sem commitar, enquanto
seguia convertendo os testes de `phxsql-store`. Ao chegar na hora de
commitar, `git log` mostrou que outra frente, trabalhando o pedido 166
(DbLink) na mesma árvore, já tinha commitado (`948e153`, 17:15:33) — e
`git show HEAD:docs/PENDENCIAS.md` trouxe **minha** linha do pedido 150
**junto** com a linha deles do pedido 166, dentro do commit deles, cuja
mensagem não menciona o pedido 150 em lugar nenhum.

## 2. O que eu concluí primeiro, e estava errado

Concluí que estava seguro porque a regra da casa — `git commit -F - --
<caminhos>`, nunca `git add -A` — existe exatamente para isolar o trabalho de
cada frente numa árvore compartilhada, e eu vinha seguindo essa regra à risca.

Estava errado: a regra protege contra pegar arquivos **de fora** do que se
pretende commitar, mas `docs/PENDENCIAS.md` é um único ARQUIVO que **as duas
frentes precisavam editar na mesma janela** — eu, a linha do pedido 150; a
outra frente, a linha do pedido 166. Quando a frente do DbLink rodou `git
commit -F - -- docs/PENDENCIAS.md docs/dossie/pedidos.html ...` (o caminho
certo, pela regra certa), o `git add` daquele caminho pegou **o arquivo
inteiro como está no disco no momento do commit** — as duas linhas juntas,
porque as duas já estavam lá. Caminho explícito isola arquivo de arquivo; não
isola hunk de hunk dentro do MESMO arquivo.

## 3. O que a medição disse

Zero conflito de conteúdo — as duas linhas (150 e 166) não se sobrepõem, e o
texto de ambas está correto no commit final. O preço não foi um bug: foi
**atribuição**. A mensagem do commit `948e153` conta a decisão do DbLink, e
não diz uma palavra sobre a bateria de testes não limpar o que cria — quem ler
o histórico procurando o pedido 150 não acha por mensagem de commit, só por
`git blame` na linha certa.

## 4. A regra

Numa árvore compartilhada, `docs/PENDENCIAS.md` (e qualquer outro arquivo que
mais de uma frente precise editar na mesma rodada, como `docs/dossie/*.html`
gerado) não é protegido por caminho explícito — é protegido por **janela de
tempo**: commite sua linha assim que ela ficar pronta, em vez de deixá-la no
disco enquanto o resto do trabalho continua. Se o commit de outra frente
chegar primeiro e arrastar sua edição junto, não desfaça (`amend`/`reset`
destruiria o trabalho real dela) — confira que o conteúdo está certo e siga;
a perda é só de atribuição na mensagem, não de conteúdo.

## 5. Como está guardado hoje

Nada a corrigir no repositório: a linha do pedido 150 está certa dentro do
commit `948e153`, e o `docs/dossie/pedidos.html` gerado reflete as duas
mudanças corretamente. Este arquivo é o registro de que a atribuição ficou
errada, para quem procurar «quando o pedido 150 foi documentado» pelo log de
commits e não achar.
