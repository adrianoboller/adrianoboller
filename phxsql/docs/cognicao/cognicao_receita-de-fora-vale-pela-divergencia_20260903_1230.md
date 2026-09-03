# Receita de fora vale mais pela divergência que pela concordância

**Descoberto em 03/09/2026, 12:30**, lendo o manual do MySQL(R) e a KB do
MariaDB(R) contra os gaps de integridade referencial.

## 1. O que aconteceu

Seis mecanismos foram perguntados aos dois manuais. Em **cinco** deles os dois
dizem a mesma coisa; em **um** eles divergem. O saldo, por tipo:

| o que os dois dizem | efeito aqui |
|---|---|
| teto de cascata em 15 níveis | confirma o nosso 16 — nada a fazer |
| sem `DEFERRABLE`, conferência imediata | aposenta uma dúvida — nada a fazer |
| cascata não dispara gatilho | vira decisão escrita — nada a construir |
| desligar a checagem deixa resíduo | é a nossa §4 — nada a construir |
| auto-referência **recusa** | **defeito nosso**, fechado no dia |
| índice criado sozinho na declaração | **terceira saída** que não vimos |
| **divergem:** reparar sozinho vs recusar | **responde o pedido 172** |

Quatro dos sete não produziram trabalho nenhum, e isso é resultado, não
desperdício: recusa medida impede a mesma proposta de voltar.

## 2. O que eu concluí primeiro, e estava errado

Entrei procurando **funcionalidade para copiar** — «o que eles têm que a gente
não tem». Foi a pergunta errada duas vezes.

Primeiro porque o que os manuais mais produziram foi o **oposto de trabalho**:
três respostas foram «nada a fazer, e agora com fonte». O valor delas é
retirarem perguntas da mesa, e eu não teria contado isso como resultado se não
tivesse escrito a tabela acima.

Segundo, e pior, porque **o único defeito que a pesquisa achou não é uma
funcionalidade que falta.** A auto-referência: eles recusam, nós fazemos
`if irma == eu { continue; }` e a subordinada fica órfã com o `atualizar`
devolvendo `Ok`. Não faltava mecanismo — o limite era consciente e estava
escrito no comentário. Faltava **a saída ser recusa em vez de silêncio**. Eu
teria passado direto por isso procurando recurso.

E a concordância, sozinha, não decide nada: os cinco pontos em que os dois
concordam confirmam o que já fazíamos. **Quem decidiu foi a divergência** — e
não por autoridade, mas porque a divergência **obriga a perguntar o motivo
dela**, e o motivo é que serve.

## 3. O que a medição disse

**A divergência, e o que ela expõe.** MariaDB repara sozinho ao abrir
(`aria_recover_options` vem `"BACKUP,QUICK"` de fábrica). MySQL/InnoDB recusa e
manda o DBA ligar `innodb_force_recovery` à mão, avisando que *«values of 4 or
greater can permanently corrupt data files»*.

Os dois estão certos, para casos diferentes — e o próprio MySQL nomeia o
critério: sem `FORCE`, o reparo automático **aborta** quando perderia linha;
com `FORCE`, completa e só avisa depois (*«Found 344 of 354 rows when
repairing»*). O risco é **perder linha**, e ele só existe quando o reparo mexe
no arquivo de **dados**.

O nosso `reindexar()` trunca o `.ndx` e o reconstrói **lendo o `.reg`**, que
está íntegro — é o `REPAIR TABLE ... QUICK`, *«will not modify the data
file»*. **Nenhuma linha pode se perder.** O risco que fez o InnoDB recusar não
existe no nosso caso, e é por isso que o lado do MariaDB é o nosso.

**E o número que fecha:** reconstruir sai a **2,2 µs por linha**, linear —
2,2 ms a mil, 219 ms a cem mil, 1,16 s a meio milhão. Medido com
`--example custo-do-reindexar-no-arranque`, três voltas por tamanho, mediana.

**O terceiro número, que é nosso e mudou a pergunta do pedido:** a recuperação
**já** reconstrói (`transacao.rs:1176`, para toda tabela nomeada na marca). O
172 não pergunta «devemos passar a reparar?» — pergunta por que o reparo não
alcança a filha da cascata, e a resposta é estrutural: o `Escrita` só nasce da
tabela pedida (`servidor.rs:8280` e `:8329`), e a cascata nunca vira `Escrita`.

## 4. A regra

**Pergunte a receita de fora onde ela DIVERGE de si mesma, e adote o lado cujo
motivo se aplica a você** — nunca o lado do motor mais parecido. Concordância
entre fontes confirma desenho e não decide nada; divergência obriga a achar o
critério, e é o critério que serve.

E o corolário do que eu errei: **procure onde a saída deles é RECUSA e a nossa
é silêncio.** É o achado mais provável de uma leitura de manual, e não aparece
procurando funcionalidade que falta.

## 5. Como está guardado hoje

`docs/INTEGRIDADE.md` §7, com citação e URL por item, e o veredito de cada um.
O defeito da auto-referência fechou no dia: pedido 174, guarda
`auto-referencia-em-silencio` **provada** nos dois sentidos, e o teste de
controle é o do comportamento **velho** — mexer na coluna local da chave
continua passando.

O que **não** está guardado: nada relê os manuais quando eles mudam. A própria
pesquisa achou que a 9.6 do MySQL(R) **moveu a cascata do InnoDB para a camada
SQL** — mudança grande, num ponto que esta seção cita. A §7 tem data no
cabeçalho e é só isso; não há gerador nem catraca por trás dela, e é o mesmo
buraco que o «37 guardas» sem data já mostrou nesta semana.
