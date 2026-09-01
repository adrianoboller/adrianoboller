# As lições

Este arquivo é o único da base que **se edita à mão** — os outros o extrator
refaz. Cada lição vem com a cicatriz que a produziu, porque regra sem o motivo
junto não gruda: alguém a lê, acha exagero, e contorna.

---

## 1. Medir

### Número citado é número que não se mede

Um medidor dizia «~20 toques de página por linha», citando um `strace` de outro
dia. Eram **10,86** — e é por isso que a conta do CRC nunca fechava naquele
documento. O medidor passou a contar os toques por dentro.

### Número digitado à mão envelhece calado

O selo da capa de um relatório passou **quatro lançamentos** dizendo a versão
errada. Havia um script justamente para impedir isso, e ele não cobria aquele
pedaço. Hoje todo número visível sai de um gerador, ou está errado e ninguém
percebeu ainda.

### A receita de um número também envelhece

Um gerador calculava o tamanho da interface a partir de uma **lista de três
arquivos copiada dentro dele**. O código passou a embutir nove, e o rodapé
publicou **780 KiB** quando a interface tinha **1.032**.

A mesma coisa aconteceu de novo, meses depois, em outro lugar: o conferidor de
textos varria cinco arquivos e o servidor entregava seis — **1.474 linhas de
interface eram invisíveis**, e o número que todos citavam era medida sobre
cinco sextos da tela.

> **Quando um gerador depende de uma lista, a lista tem de sair do código.** E
> uma guarda tem de reprovar quando as duas divergirem.

### Medidor com binário velho mede o passado

`cargo build --release` não recompila os *examples*, e a bancada chamava o
binário direto. Uma rodada inteira de ganhos ficou invisível — e a conclusão «o
esquema custa 2,2×» nasceu, com tabela e tudo, dessa diferença.

Variante do mesmo erro: um script de bancada apontava para o binário de **outra
árvore** por caminho absoluto.

### Medir a premissa do item vem antes de implementar o item

Um pedido dizia «ordene as chaves do lote antes do índice» e vinha com o alvo
certo — o gargalo estava mesmo lá. Só que a causa era outra: não era
localidade, era reler e recalcular o CRC da **mesma página** a cada descida da
árvore. A desordem custava 1,06×; ordenar teria comprado quase nada. Um cache
de páginas comprou **2,40×**.

Vale inclusive quando a premissa é «isto é pré-requisito daquilo» — um item
deixou de ser pré-requisito quando o desenho foi escrito.

### Diagnóstico plausível não é diagnóstico medido

Escrevi que «o mutex era o pior pedaço, porque serializa». Medido: o `lock` sem
disputa custa **13,2 ns** e o parse do lote custa **3.456 µs** — 262.000× mais.

> **O diagnóstico errado sobrevive melhor quando o conserto funcionou por outro
> motivo.**

### Hipótese que morre medida é resultado

Acorde o **critério de morte antes de medir** («abaixo de 2× a hipótese
morre»), e escreva a recusa com o número. Recusa medida é o que impede a mesma
ideia de voltar sem medição.

### Comparar trabalho igual, não só pergunta igual

Dois erros opostos, os dois em bancada contra outro motor: um `WHERE id IN (…)`
contra vinte mil buscas separadas (41× a favor do outro), e um `COUNT(*)+SUM`
sobre 1.250.000 linhas contra a leitura de 20.000 (5× a favor do nosso).
**Nenhum dos dois aparecia no número.**

Um terceiro: comparar um motor que agrupa as escritas num `COMMIT` contra outro
que faz vinte mil gravações separadas.

---

### Comparar três é diferente de comparar dois duas vezes

Havia duas bancadas — PhxSql × MySQL(R) e PhxSql × SQLite(R) — e a tentação era
somar as tabelas. Somar dá **três colunas e nenhuma comparação**: as medidas
são de dias diferentes, com cargas diferentes na máquina, e parte da diferença
passa a ser do ambiente em vez do motor. É o mesmo erro de comparar escalas
diferentes, com outra roupa. Os três correm **intercalados na mesma rodada**.

E montar a terceira achou o que duas não achavam: a bancada do MySQL(R) gravava
`'2024-10-04'` em toda linha enquanto as outras duas gravavam o dia variável.
**Dado diferente, do mesmo tamanho, invisível em qualquer medida de tempo.** O
que o achou foi ter de conferir três em vez de dois.

### Quando os lados não têm a mesma forma, meça o piso da forma

O SQLite(R) é biblioteca em processo; o MySQL(R) é daemon que recebe texto por
soquete. Não há como igualar — não existe MySQL(R) embutido. O que se faz é
medir o piso: 20.000 instruções que **não fazem nada** custam 1,479 s, que são
**59,6% da barra de busca dele**.

Sem esse número teríamos publicado «15,16× mais rápido»; entre motores são
**6,12×**. Mais da metade da vitória era do formato. **Vitória que vem do
formato é a mentira mais convincente que existe.**

### Não há vencedor quando as faixas se cruzam

164 ms contra 166 ms, com as faixas em 151–215 e 158–232. Marcar um dos dois
como vencedor é publicar ruído da máquina como resultado. O desenho passou a só
contornar quando o **máximo do primeiro é menor que o mínimo do segundo**.

E a regra tem de morar num lugar só: eu a consertei no gráfico e a tabela do
dossiê continuou marcando vencedor na busca — **o documento se contradizia a
dois centímetros de distância**.

### Uma rodada fora da curva não pode mandar no eixo

O `atualizar` do MySQL(R) foi 22,97 s numa rodada e 3,48 s na seguinte. Com o
eixo ancorado no **máximo**, as barras de 277 ms e 1,03 s viravam lascas de 3
px: uma rodada esmagava três painéis. O eixo passou a ser a maior **mediana**,
com o bigode cortado por uma seta quando estoura — a excursão continua dita, no
rótulo, em vez de mandar no desenho.

## 2. Provar

### Prova real é nos dois sentidos

**O teste tem de FALHAR com o defeito reposto e passar com o conserto.** Teste
que passa por engano é pior que teste que falta, porque dá a sensação de
cobertura.

Um caso vivido: a prova de um teto de memória **passou com o defeito reposto**,
porque conferia o *veredito* e a conferência acontecia **depois** da leitura —
acusava mesmo sem a proteção, só que aí a memória já tinha sido gasta. O teste
passou a medir **quanto** foi lido: com o defeito, 10.001 bytes num teto de 64.

### `| tail` mascara o código de saída — e o portão passa a mentir verde

`cargo test --workspace 2>&1 | tail -25` guardou 25 linhas e devolveu o código
de saída do **`tail`**, que é sempre 0. O portão dizia «passou» sem que ninguém
tivesse olhado o resultado, e o arquivo de saída não tinha o resumo para
conferir. Rodar de novo sem cano deu 1.436 testes em 46 binários e o zero de
verdade.

É a mesma família do teste que passa por engano: **o canal que resume também
descarta a prova.**

### O que depende do sistema operacional se prova contra o sistema operacional

Dez testes unitários passavam, e a prova por soquete mostrou que a queda da
conexão **não soltava a reserva**. A causa não estava no servidor: era o teste
— `socket.makefile()` do Python segura o descritor, e fechar só o soquete
deixa o fd aberto, então o servidor nunca via o fim da conexão.

### Guarda que troca travamento por erro enfraquece o teste que dependia do travamento

Um conserto fez um defeito parar de pendurar e virar um `return` silencioso —
e com isso um teste cujo único sintoma era o travamento passou a **não pegar
mais**. Ele ganhou a asserção da *consequência*, e reprova em 0,12 s em vez de
30.

### Defeito que trava precisa de prazo no teste

Sem prazo, o teste trava junto e a bateria inteira para.

---

## 3. Integrar trabalho de vários agentes

### Há defeito que só existe no encontro das frentes

Numa rodada de seis frentes, **três defeitos só apareceram na integração**:

1. Uma frente pôs um teto de memória num `read_line`; a outra trocou aquele
   `read_line` por um canal cifrado que lia **sem teto**. Juntar as duas como
   vieram devolveria a leitura ilimitada — **e não havia conflito de merge
   nenhum**.
2. A bateria ficou com **dezoito** partes quando cada frente contou dezessete:
   cada uma somou as suas.
3. O relatório perdeu **uma seção inteira** porque o merge escolheu o lado de
   quem não a tinha.

### Arquivo gerado se resolve regerando — se ele também tiver texto escrito à mão, não

Nesse caso, escolher um lado apaga o texto do outro **sem conflito aparecer**.

### Merge de conflito marca quem MEXEU, não quem perguntou por último

Deixar «o meu» em todas as colunas desfaz em silêncio o trabalho do outro nas
colunas que ninguém tocou. O padrão certo é por coluna.

### Resolver catálogo e lista por IDENTIDADE, não por linha

Guardas, entradas de catálogo e itens numerados são **acréscimos** de frentes
diferentes: descartar um lado apaga a prova que ele carrega. Compare por id, e
compare contra o estado **atual**, não contra a base do merge — a base pode ser
anterior a três integrações.

### Números escritos à mão que o compilador confere são guardas de graça

Dois vetores de tamanho fixo pegaram um merge incompleto: o número não bateu e
não compilou.

### Colisão de numeração é rotina

Duas frentes reivindicaram o mesmo número de pedido; itens apareceram
duplicados com contas diferentes. Tenha um gerador que **detecte duplicata** e
rode-o em toda integração.

---

## 4. O ambiente

### Nunca matar processo por nome

`pkill -f` **matou o meu próprio shell** (saída 144). Mate por PID, e confira
antes o `cwd` do processo para saber se é seu.

### Não é preciso VM para provar outra arquitetura

A máquina não tinha `/dev/kvm` nem flag de virtualização — VM completa estava
fora. Mas `qemu-user-static` emula o **binário**, não a máquina, e não depende
de KVM. Um documento que dizia «os binários ARM nunca foram executados» durou
uma hora: com o emulador, o servidor ARM subiu, autenticou e **gravou e leu 50
linhas**.

> Antes de dizer «não dá para testar», pergunte se o que falta é emular a
> máquina ou só o binário.

### O temporário de teste é uma fuga de disco silenciosa

A bateria deixou **80.088 diretórios em `/tmp`, 6,4 GB**, o mais antigo de três
dias. O ambiente chegou a 560 MB livres e ninguém sabia por quê.

Duas coisas sobre o zelador que resolveu isso:

- **Nada é apagado sem antes se provar que nenhum processo vivo está usando
  aquilo** — por caminho real (`cwd`), nunca por data ou nome. Zelador que
  apaga o `target` de quem compila não economiza espaço, perde uma rodada.
- Conferir oitenta mil diretórios chamando `/proc` um a um **estoura o tempo**.
  A lista de processos se levanta uma vez.

E o total que ele imprimia estava errado: somava só a parte que o shell apagou
e dizia **362 MiB** numa corrida que liberou quase 10 GB. Passou a sair da
diferença medida no próprio disco.

### A causa-raiz continua sendo do teste

O diretório temporário tem de morrer com o teste que o criou — guarda que apaga
no `Drop`, **não** um `rm` no fim do corpo: teste que falha no meio nunca chega
ao fim.

---

## 5. Guardas e catracas

### Catraca só desce, e catraca frouxa não segura nada

Um contador de dívida (textos por traduzir, avisos por resolver) vira guarda se
o teste reprovar quando ele **sobe** — e também quando alguém melhora e
**esquece de baixar o número**.

### Subir a catraca só se justifica quando o número de baixo era falso

Aconteceu uma vez: o número não cobria a interface inteira. Isso não é
afrouxar, é corrigir uma leitura — e o motivo tem de ficar escrito ao lado.

### Toda guarda com o defeito que a motivou, provada periodicamente contra ele

Um catálogo de guardas onde cada entrada guarda o trecho original e a troca que
repõe o defeito. Roda-se a mutação e conta-se quantas **não pegaram**.

### O portão de permissão é UM só — e o campo que ele lê é o furo

O direito por tabela conferia o campo `"tabela"` do pedido. Duas operações não
têm esse campo: uma guarda as tabelas em `a.tabela`/`b.tabela`, outra numa
lista. Bastaria pedir a tabela negada como o lado B de uma junção. Apareceram
**quatro furos** do mesmo tipo.

> **Quando o portão passar a olhar um campo novo, procure quem não tem esse
> campo.** E não espalhe o portão por quarenta operações: a que alguém esquecer
> vira a porta dos fundos.

### O teste que mais importa numa regra nova é o do comportamento VELHO

`sem_regra_de_tabela_nada_muda`. Regra que muda o significado da configuração
existente tira o direito de alguém sem ninguém ter pedido.

### Guarda nova entra pedida, não imposta

Uma proteção que recusa todo cliente escrito antes dela **não é proteção, é
estrago**. Quem pede ganha a garantia; quem não pede continua como antes; e o
administrador que quer obrigatoriedade liga a chave.

O corolário de segurança: quando a proteção é opcional, **escreva em palavras
claras o que ela vale desligada** — «isto protege contra escuta passiva e nada
mais» — em vez de deixar parecer mais.

---

## 6. Interface

### Interface só se prova exercitando

Gravar um vídeo de demonstração achou **três defeitos em cinco minutos** que
ler o código não acharia — e o pior quebrava todo salvar e todo incluir.

### O CSS global morde todo componente novo

`input{width:100%}` transformou um rádio numa bolinha do tamanho da célula.
`label{text-transform:uppercase}` fez «Blumenau» aparecer como «BLUMENAU» — que
é **mentira sobre o dado**, porque quem olha não sabe se está gravado assim.

> **Rótulo se estiliza e se traduz; dado, nunca.**

### Coluna de sistema nova quebra quem filtra pela primeira

Quando entrar uma peça no fim de uma lista, procure quem usa `find(...)` onde
devia usar `filter(...)`.

---

## 7. Texto de tela e tradução

### Texto se resolve por CHAVE, nunca por comparação da frase

No dia em que alguém melhorar a redação, quem compara frase quebra **calado**,
mostrando o idioma de origem.

### Chave morta é pior que chave faltando

O tradutor a vê na tabela, traduz nos seis idiomas, e nada muda na tela. Teste
para os dois lados do laço: chave que a tela pede e não existe, e texto que
existe e ninguém pede.

### Frase picada por marcação é intraduzível por construção

`"funciona em"` + `<b>qualquer navegador</b>` + `"— é layout"` não se traduz em
pedaços: a ordem das palavras muda de idioma para idioma, e em alemão o verbo
vai para o fim. **A frase inteira vira uma chave**, com a ênfase como marca
dentro do texto, e o corte em etiquetas acontece **depois** da tradução.

### «Igual ao original» não é sinal de «não traduzido»

Uma guarda ingênua reprovaria `Database`, `Profiler`, `Pivot` e `Menu
principal` — que em francês é exatamente isso. O caso que denuncia de verdade é
**todos os idiomas idênticos**, ou uma frase longa repetida em três ou mais.

---

## 8. Formato em disco

- **Mudança de formato entra cedo.** Enquanto não há dado em produção é barata;
  depois vira migração.
- **A ordem de digitação é sagrada.** Um endereço que nunca se reaproveita
  torna a replicação fiel linha a linha e a leitura O(1) — e é a garantia que
  qualquer proposta de versionamento múltiplo precisa enfrentar antes de ser
  aceita.
- **Configuração que não é lida mente.** Um campo esteve no arquivo, no manual
  e na tela por três versões, e **nenhuma linha de código o lia**. Campo sem
  leitor é pior que campo ausente: o ausente ninguém ajusta esperando efeito.
- **Slot com CRC torna escrita rasgada detectável** — o que muda o que é
  preciso fazer contra corrupção.

---

## 9. Trabalhar com agentes

### O briefing carrega a regra, não só o pedido

Todo briefing desta sessão leva junto as armadilhas que o projeto já pagou. É
isso que faz o agente não repetir o erro — e está tudo em
`02-BRIEFINGS-DE-AGENTE.md`, que é o ativo mais reaproveitável desta base.

### Peça o documento antes do código quando o desenho decide

Nas frentes de risco (transação, criptografia, formato), o briefing manda
escrever o documento com as decisões **antes** de implementar, listando as
perguntas que ele precisa responder.

### Acorde o critério de morte antes da medição

Senão o número vira defesa da hipótese em vez de teste dela.

### Peça a saída, não a afirmação

«Mostre a saída de cada defeito reposto» em vez de «confirme que testou». Dois
agentes reportaram, por conta própria, testes que passavam por engano.

### Faixa de portas por agente, e nunca matar processo alheio

Vários agentes na mesma máquina sobem servidores. Sem faixa reservada, um
derruba o outro por «porta em uso».

### Um agente pode recusar entregar metade

Uma frente devolveu o terreno pronto e **recusou** entregar meia transação,
dizendo que meia era o pior resultado possível. Estava certa.
