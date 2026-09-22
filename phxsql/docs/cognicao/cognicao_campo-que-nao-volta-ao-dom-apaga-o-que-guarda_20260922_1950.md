# Cognição: campo que não volta ao DOM não se lê sem condição — ele APAGA

**Descoberto em** 22/09/2026, 19:50 UTC, percorrendo o assistente de
replicação com o Playwright (pedido 379).

## 1. O que aconteceu

O passo 2 do assistente tem seis campos, e cinco deles voltam para o HTML com
o valor guardado (`value="${esc(rz.host)}"`, `value="${esc(rz.token_remoto)}"`
…). O sexto, a **senha do usuário da origem**, não volta — e não voltar é uma
decisão certa: credencial no DOM é credencial que sai num «salvar página».

O passo 2 lia os seis do mesmo jeito:

```js
rz.senha = $("#rzSen").value;
```

Para os cinco que voltam, ler sem condição é ler o que está lá. Para o sexto,
ler sem condição é **apagar**: o campo nasce vazio a cada pintura da tela.

O caminho que quebra é o mais comum do assistente inteiro. A sonda do passo 3
falha (porta errada, cifra desmarcada, o que for), a pessoa clica em
«← Corrigir a conexão», conserta o que estava errado e clica em «Testar a
conexão» de novo — e a sonda volta com **«usuário ou senha inválidos»**, uma
credencial que estava boa e que a própria tela acabou de zerar.

## 2. O que eu concluí primeiro, e estava errado

Que era **erro do meu roteiro**: «esqueci de preencher a senha». Cheguei a
acrescentar `page.fill('#rzSen', SENHA)` antes da primeira sonda — e o erro
continuou exatamente igual, porque o roteiro *já* preenchia e o preenchimento
morria na volta.

O diagnóstico plausível seguinte foi **«o token não autentica»**, porque a
primeira falha que vi dizia `faca login antes`. Esse era outro assunto e era
verdade: source com cadastro de usuários exige login, e o token sozinho não
abre o `bancos` (`servidor.rs`, o portão do `erro.faca_login`). Consertar
aquilo desmascarou este — o erro mudou de «faça login» para «usuário ou senha
inválidos», e só então a causa ficou visível.

Dois diagnósticos plausíveis em sequência, e o conserto do primeiro foi o que
revelou o segundo. **O errado sobrevive melhor quando o conserto funcionou por
outro motivo.**

## 3. O que a medição disse

Não é número, é o par de estados, medido no navegador:

| passagem pelo passo 2 | `#rzSen` no DOM | `rz.senha` depois do clique |
|---|---|---|
| primeira (a pessoa digita) | `segredo1` | `segredo1` |
| segunda (voltou de «Corrigir») | **vazio** | **vazio** — apagou |

E a prova de que a causa era essa: com `if ($("#rzSen").value)` na frente, a
mesma sequência de cliques, **sem redigitar nada**, passou de
«Não conectou — usuário ou senha inválidos» para «Conectou».

## 4. A regra

**Campo que você decidiu não devolver ao DOM deixou de ser fonte da verdade:
lê-lo sem condição não guarda, apaga.** Vazio nele quer dizer «mantenha o que
eu já sei», não «esvazie» — e a tela tem de DIZER isso, senão a pessoa acha
que a senha se perdeu. Quem quiser mesmo apagar troca o usuário.

E o corolário de quem procura o defeito: quando dois campos vizinhos se leem
com a mesma linha, **olhe se os dois voltam para o HTML**. O irmão aqui é o
`#rzTokenR`, que é `type="password"` igual e **tem** `value` — a diferença não
está no tipo do campo nem no nome dele, está em quem repinta o valor.

## 5. Como está guardado hoje

- **O conserto**, com o motivo acima da linha, em
  `crates/phxsql-server/ui/index.html`, passo 2 do `assistenteReplicacao`:
  `if ($("#rzSen").value) rz.senha = $("#rzSen").value;`
- **O que a tela diz**: marcador d'água pela fábrica de idiomas,
  `tela.rz_senha_guardada` — «guardada — deixe em branco para manter» —, e ele
  só aparece quando há senha guardada.
- **A guarda, nos dois sentidos**, em `testes-web/exercitar-fio-e-rele.mjs`:
  o roteiro volta de uma sonda falha e avança de novo **sem redigitar a
  senha**. Tire o `if` e a linha «a mesma sonda CIFRADA e com pino conecta»
  falha com `usuario ou senha invalidos`. O roteiro ainda afirma que o campo
  voltou vazio e que o marcador d'água está lá — as duas metades da decisão.
- **Onde o buraco ficou:** é UMA tela. Não varri as outras da página atrás de
  `campo type="password"` sem `value` lido sem condição, e esse é o casador
  que acharia os irmãos. Fica nomeado aqui em vez de sumir do relatório.
