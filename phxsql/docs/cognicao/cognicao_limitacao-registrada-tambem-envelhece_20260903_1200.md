# Limitação registrada também envelhece — e a prova ao lado é o que a conserva

**Descoberto em 03/09/2026, 12:00**, ao rodar um `push` que eu vinha dizendo,
a sessão inteira, que não funcionava.

## 1. O que aconteceu

`git push -u origin claude/capacidades-disponiveis-y6auxh` **funcionou**:
`d4dc424..cd561ce`, **248 commits**. O `origin` tem hoje **391**, e a
confirmação não é a palavra do `git` — é `git ls-remote`, que pergunta ao
GitHub e devolve `cd561cec…` para a branch, idêntico ao `HEAD` local.

Até esta corrida, três documentos afirmavam o contrário no presente do
indicativo: o `CLAUDE.md` («este papel está degradado hoje»), o
`docs/BACKUP.md` inteiro, e o pedido 18 do `PENDENCIAS.md`.

## 2. O que eu concluí primeiro, e estava errado

Não é que eu tenha diagnosticado mal — o diagnóstico estava **certo e medido**,
e é essa a parte incômoda. O `docs/BACKUP.md` separou as três causas possíveis
(proxy, credencial, GitHub), comparou `git-upload-pack` com `git-receive-pack`
no mesmo host e na mesma sessão TLS, e mostrou o 403 com `X-Github-Request-Id`
e o `Content-Type` que só o GitHub carimba.

O erro foi outro, e mais sutil: **tratei a medição como permanente.** Passei a
sessão repetindo «o `push` recusa com 403» — inclusive num relatório desta
mesma rodada, ao classificar o papel do versionador como degradado — sem
tentar uma única vez. E o que me fez tentar não foi raciocínio: foi o *stop
hook* pedindo o `push`, e eu decidindo medir antes de responder «não dá», como
a casa exige para qualquer número.

**A prova ao lado foi o que conservou o erro.** Uma limitação afirmada sem
medida alguém põe em dúvida na semana seguinte; uma limitação com request-id e
tabela de endpoints ninguém retesta — ela parece resolvida, e o que está
resolvido é só o *diagnóstico*, nunca o *estado*.

## 3. O que a medição disse

| | antes (02/09) | agora (03/09, 12h) |
|---|---|---|
| `git-upload-pack` (ler) | funciona | funciona |
| `git-receive-pack` (escrever) | **403 Forbidden** | **funciona** |
| commits no `origin` | 143 | **391** |

Nada mudou do lado do código: o acesso de escrita foi concedido entre uma
medição e a outra. E o custo do atraso é contável: **três rodadas** de backup
saindo por pacote entregue à mão, e duas prioridades da 0.19.0 (release
reproduzível e CI) paradas esperando exatamente isto.

## 4. A regra

**Limitação que bloqueia um papel se remede a cada rodada, com a receita que o
próprio documento carrega.** Diagnóstico não vence; estado vence. E quanto
melhor a prova que acompanha a limitação, mais cedo ela precisa de nova data.

## 5. Como está guardado hoje

O `docs/BACKUP.md` já trazia a receita de reconferência (`git push --dry-run`,
que exerce o mesmo `git-receive-pack` sem mexer em nada) — ela existia e
ninguém a rodava. Agora o documento abre pelo **estado**, com data, e a
descrição antiga está marcada como história.

O que **não** está guardado: nada obriga a rodar a reconferência. As catracas
desta casa vigiam número que sobe; **não há catraca para afirmação que
envelheceu**, e esta é a segunda vez na mesma semana que uma frase no presente
do indicativo ficou velha em silêncio — a primeira foi o «37 guardas» sem data.
As duas pedem a mesma coisa e ainda não a têm: **data ao lado da afirmação, e
alguém que releia as datadas.**
