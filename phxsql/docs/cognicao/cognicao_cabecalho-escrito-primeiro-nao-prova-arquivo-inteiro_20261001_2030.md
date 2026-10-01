# O cabeçalho escrito primeiro não prova que o `*.novo` está inteiro — e «duas fases» só vale se TODO caminho as segue

**Estado:** PENDENTE

## O que aconteceu

Pedido 632, achado por leitura na frente do 624 e medido aqui pela queda real
(`Ponto::NoMeioDoNovoDoVolume`, armado na 2ª passagem por `armar_na`). O
`RegFile::regravar_esquema` de uma fase só — o que o `acrescentar_indices`, a
chave e a marca sem troca preparada usam quando o bloco de esquema não cabe
antes do slot 1 — chamava `reescrever_volume(.., trocar = true)` volume a
volume: escrevia o `*.novo` do volume 1, renomeava, e só então escrevia o do
volume 2. O pedido 422 já tinha separado FASE A e FASE B para o caminho fora da
trava; o de uma fase só ficou com o laço antigo.

## O que eu concluí primeiro, e estava errado

Duas hipóteses antes de medir:

- **H1 (viva):** o defeito existe — com o volume 1 já trocado, o `*.novo` do
  volume 2 cortado depois do cabeçalho se declara com a geometria nova, e o
  `terminar_troca_interrompida` o renomeia por cima do volume velho.
- **H2 (morta):** a troca em duas fases dos pedidos 605/625 já cobria o caso.
  Morreu na leitura do chamador e na queda: as duas fases cobrem o
  `alargar_fase_a/b` e o `regravar_esquema_fase_a/b`; o caminho de uma fase
  não passava por nenhum dos dois.

E a conclusão errada que quase entrou junto com o conserto: «basta o caminho de
uma fase seguir as duas fases». Isso conserta o caminho que motivou, mas a
abertura continuava acreditando em qualquer `*.novo` de cabeçalho válido — e o
cabeçalho é **a primeira coisa** que o `reescrever_volume` e o
`alargar_fase_a` escrevem. O cinto da abertura entrou junto, com prova própria.

## O que a medição disse

- Defeito reposto: o `clientes#002.reg.novo` de **1.088 bytes** (cabeçalho e
  esquema, zero slot) renomeado por cima do `clientes#002.reg` de **3.900** —
  as **30 linhas** do volume 2 destruídas no disco — e a tabela **sem abrir**
  («ficou pela metade» no volume 3).
- Só o laço de antes, com o cinto: volume 2 intacto, mas a tabela também não
  abre — o cinto troca perda por recusa; quem faz o teste passar é a ordem.
- Só sem o cinto (`*.novo` do volume 3 encurtado à mão numa troca decidida
  real): a tabela ABRE e a varredura morre em «failed to fill whole buffer».
- A mensagem da recusa dizia «o volume 1 declara slot de 98 bytes e este
  declara 98» e mandava «reponha o `*.novo`» — o arquivo incompleto. Hoje diz
  qual campo diverge e, havendo `*.novo` incompleto, que ele **não** entra.

## A regra

Quando o arquivo temporário começa pelo cabeçalho, confira o tamanho — não o
cabeçalho — antes de deixá-lo entrar no lugar do arquivo vivo; e quando um
caminho ganha duas fases, procure o irmão que ainda escreve e troca no mesmo
passo.

## Como está guardado hoje

- `crates/phxsql-store/tests/troca-interrompida.rs::a_queda_no_meio_do_novo_do_volume_2_nao_perde_linha`
  e `::a_troca_decidida_nao_renomeia_novo_incompleto`, as duas medidas RED com
  o defeito reposto.
- Guardas `regravar-esquema-troca-volume-a-volume-632` e
  `troca-decidida-renomeia-novo-incompleto-632`, provadas.
- O `reescrever_volume` perdeu o parâmetro `trocar`: não há mais como escrever
  e trocar no mesmo passo por ele.
- **Efeito colateral na régua da trava:** a `alcancam-fsync-2` desceu de 23
  para 22, e a descida é da régua (3 saltos), não do código — a
  `op_excluir_fk` continua com a trava sobre `fsync`, só que dois saltos além
  do corte. Medido com `--json` no HEAD de antes e nesta árvore; nota no
  próprio `mapa-da-trava.py`.
- **Buraco que fica:** a contagem de slots pega o arquivo CORTADO, não o
  adulterado no meio (esse o CRC de cada slot acusa na leitura). E o volume
  velho de cabeçalho ilegível só confere se o `*.novo` não termina em slot
  pela metade.
