# Texto de erro só se redige na origem — o sumidouro não tem como analisar

**Estado:** PENDENTE

## O que aconteceu

Pedido 497 (B1 do parecer SEC do 495): o `acessos.log` gravava o
`e.to_string()` da recusa, e a recusa citava o literal do pedido. A casa já
tinha redação por análise (`profiler::redigir`, `usuario::sem_a_senha`) — mas
as duas analisam o **pedido**, e o que ia ao log era o **erro**. O mesmo texto
sai por oito chamadas ao `anotar`, pela resposta, pelo Profiler e pelos jobs.

## O que eu concluí primeiro, e estava errado

1. Que o conserto era no `anotar`, como o pedido dizia («conserto no caminho
   que motivou»). Não há como: a frase chega montada, e a única redação
   possível ali seria recortar — ou tirar do texto as cadeias do pedido, que
   falha no que foi transformado (`O'Brien` vira `'O''Brien'` na expressão
   normalizada).
2. Que o vazamento pelo SQL era do tradutor (`phxsql-sql`). Medido: `SELECT n
   FROM t WHERE n = 1 'x'` **passa** pelo tradutor, vai ao motor como
   expressão, e quem recusa é o `phxsql-core` — citando a expressão inteira.
   O caminho do SQL até o log era a expressão, não o `descrever`.
3. Que o `sem_a_senha` e o erro de sintaxe respondiam a mesma pergunta, e por
   isso os amarrei no `descrever` como «uma decisão só». A SEC bloqueou, com
   medição: a senha escrita `PASSWORD "x"`, `PASSWORD x` ou `PASSWORD 123` saía
   inteira no `perfil.txt`, e o job a gravava no `jobs.json`. O `descrever`
   responde «como um literal sai daqui». O `sem_a_senha` responde «onde está a
   senha»: tudo o que vem depois de `PASSWORD`, de qualquer tipo. A lei «função
   não se duplica» vale para a mesma PERGUNTA, e unificar duas perguntas foi o
   defeito.
4. Que o caso estava fechado com oito caminhos. A SEC achou outros quatro da
   mesma família: o pedaço de senha que sobra, o `fim` do cadastro, a duração
   sem teto e o valor entre aspas duplas. A família é «texto do pedido dentro
   da frase de erro», e não «literal entre aspas simples».
5. Que bastava consertar a redação. Na segunda volta, a SEC mostrou que o
   **portão** dela recortava: «as duas primeiras palavras são `CREATE USER`?»,
   por `split_whitespace`. O comentário do ODBC o enganava, e o `ALTER ROLE` e
   o `SET PASSWORD FOR` nem passavam por ele. Redação por análise atrás de um
   portão que recorta é recorte do mesmo jeito. A pergunta certa do portão
   também é «onde está a senha» (há o símbolo `PASSWORD` ou `IDENTIFIED`?), e
   não «que comando é este?».
6. Que o portão pelos **símbolos** fechava a segunda volta. A terceira volta
   mostrou que ele lê só o que o léxico vê, e o arquivo guarda os **bytes**:
   comentário, `/*!…*/`, `MASTER_PASSWORD` e o literal que carrega a senha
   passavam. O portão de quem guarda bytes pergunta pelas **letras**. A
   análise fica para o que se mostra depois que o portão disse sim. E o valor
   do `?` mora num campo irmão, que a redação da frase nunca via.
7. Que a guarda do job valia só onde o job **chega**. Ela morava no
   `Job::de_json`, que também **lê** o `jobs.json` no arranque. Endurecer a
   guarda endureceu a leitura junto, e o job legítimo `SELECT login,
   password_hash FROM contas`, salvo antes, derrubava o servidor (rc=1, quarta
   volta da SEC). Guarda que mora num leitor comum alcança o arquivo que já
   existe. Ela se separa em dois pontos: recusar onde o dado entra, anotar
   onde ele volta do disco.

## O que a medição disse

Com os defeitos repostos, o teste pelo soquete (`tests/erro-no-acessos-log.rs`)
listou **24 vazamentos**: seis caminhos × duas portas (dados e web) × resposta
e log. Os três casos «sem fechar» do SQL nunca vazaram — o léxico do SQL já
dizia só a coluna. A linha que o SEC citou, reposta como era, vaza **4** (duas
portas × resposta e log). Na volta do bloqueio, com B1, B2, P1 e P2 repostos
juntos, o mesmo teste (agora com dezesseis caminhos e o Profiler ligado) lista
**26** vazamentos. O B2 só aparece no `perfil.txt`: a resposta e o log estavam
limpos, e um teste que olhasse só os dois não o veria. Na terceira volta
(trinta e dois caminhos), com o `parametros` sem tapa, o portão pelos
símbolos, só a palavra exata abrindo a redação e o eco cru repostos juntos:
**20** vazamentos, quatro deles na resposta de um `SELECT` que rodou.
Na quarta volta, com a recusa reposta na leitura, caem os três testes do
comportamento velho, com o mesmo erro medido pela SEC. Pelo binário, o
`jobs.json` do parecer sobe, e a marca aparece **0** vezes.

## A regra

Quando o texto de um erro vai a lugar que guarda, redija o literal **onde a
mensagem se monta** — é o único lugar que sabe o que é literal; depois disso,
o texto só vira tamanho.

## Como está guardado hoje

`phxsql_core::error::LITERAL_REDIGIDO`, usado pelo `Token::descrever` (SQL, que
também tapa o nome entre aspas duplas), pelo `Token::mostrar` e pelo
`Valor::descricao` (expressão). O `usuario::redigir` é o motor próprio de «onde
está a senha», e atende ao Profiler, ao eco da op `sql` e ao job, atrás do
portão `usuario::menciona_senha`, que pergunta pelas letras. O job recusa no
`job_salvar` e no `executar_job`, e a leitura do disco só anota
(`Job::do_disco`). Dezesseis guardas no
catálogo, provadas à mão uma a uma e **ainda não julgadas pelo
`provar-guardas.py`** — a tabela do `docs/TESTES.md` diz isso. O que fica
aberto: o valor que É o diagnóstico continua citado (pedido 453), o dado
pessoal curto nele é o 464, e o símbolo culpado sem teto na frase é o 462.
