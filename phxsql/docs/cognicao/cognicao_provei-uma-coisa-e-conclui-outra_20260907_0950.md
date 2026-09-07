# Provei uma coisa e concluí outra — duas vezes no mesmo dia

**07/09/2026, 09:50 UTC.**

## 1. O que aconteceu

O dono perguntou se existe cadastro dos nós, tela de gestão, e onde se define
o quórum mínimo. Lendo a interface, achei que o assistente de replicação chama
`api("replicacao_configurar", …)`, e que essa operação não aparece em lugar
nenhum do servidor.

Escrevi **«achado sério»**, subi um `phxsqld` e provei contra o motor vivo:

```
{"ok": false, "erro": "[SP000018] nao encontrado: operacao desconhecida:
 replicacao_configurar", "nome": "NAO_ENCONTRADO"}
```

A prova estava certa. **A conclusão que eu tirei dela, não.**

## 2. O que eu concluí primeiro, e estava errado

Que o botão «Aplicar a configuração» estava quebrado — *botão que parece
funcionar e não funciona*, que é o defeito que esta casa diz custar mais caro
que botão faltando.

Fui ver o ponto de chamada, e ele está **guardado de propósito**:

```js
try { await api("replicacao_configurar", …); passoPronto(); }
catch (e) { if (e && e.nome === "NAO_ENCONTRADO") return aplicarPeloConfig(); … }
```

Com o comentário logo abaixo dizendo o desenho inteiro: *«O caminho de HOJE: o
phxsqld ainda não aplica replicação em execução — a operação
`replicacao_configurar` chega com o motor novo. O assistente não finge: entrega
o bloco exato, manda reiniciar e CONFERE antes de dar por pronto.»*

O `NAO_ENCONTRADO` que eu medi **é o gatilho do caminho de hoje**. Eu provei o
sintoma que o desenho usa como sinal, e o li como falha.

**Duas afirmações diferentes:** «a operação não existe» e «o botão quebra». A
primeira eu medi; a segunda eu supus, e ela é a que teria virado defeito
publicado.

## 3. E isso foi a SEGUNDA vez hoje

De manhã, publicando o dossiê, um aviso disse que a página oferece download
inerte. Contei `grep -c '<a [^>]*download'`, recebi **2**, e escrevi que havia
dois links. Eram os **comentários** explicando que não há nenhum.

O padrão é o mesmo nos dois, e é ele que vale guardar: **medi um proxy e li o
proxy como a coisa.** A contagem de texto não distingue *fazer* de *falar sobre
fazer*; a ausência de uma operação não distingue *quebrado* de
*compatível com o motor futuro*.

## 4. O que a medição disse, depois de perguntar direito

| pergunta | resposta medida |
|---|---|
| `replicacao_configurar` existe? | **não** — `operacao desconhecida`, contra o motor vivo |
| o botão quebra? | **não** — o `catch` desvia no `NAO_ENCONTRADO` |
| há cadastro de nós? | **sim**, três listas: `replicacao.origens[]` (da réplica), `replicacao.replicas_autorizadas[]` (ACL de IPs) e `cluster.nos[]` (todos os nós; a maioria é contada sobre ela) |
| há tela? | **Replicação sim; cluster não** — a palavra `cluster` aparece **0** vezes na interface, e das ops dela a tela chama só `spare_promover` |
| há campo de quórum mínimo? | **não existe** — nem config, nem protocolo, nem tela |

E a resposta certa à última corrigiu **o que eu havia escrito uma hora antes**
no `REPLICACAO.md` §19: eu disse que o quórum seria «política por origem», e
`origens` é a lista da **réplica**. Quem espera o quórum é o **master**, que
não tem `origens` — o campo mora no bloco `cluster`, ao lado de `nos`.

## 5. A regra

**Antes de chamar um resultado de defeito, pergunte se alguém já o usa como
sinal.** Ausência medida é fato; «isto está quebrado» é interpretação — e entre
os dois cabe um `catch` com o nome do erro dentro.

E o teste barato que teria evitado as duas: **procurar quem LÊ o sintoma**. Um
`grep` pelo nome do erro no lado que chama, antes de escrever a palavra
«defeito».

## 6. Como está guardado hoje

- O pedido 208 registra o cadastro, a meia-tela e o campo que não existe — com
  a armadilha escrita junto, para ninguém «consertar» o assistente.
- O `REPLICACAO.md` §19 traz a correção do «por origem», com a tabela das duas
  listas do lado do source e por que só `cluster.nos` serve de M.
- **Onde o buraco fica:** eu ainda não tenho um hábito que dispare sozinho.
  As duas vezes de hoje foram pegas por eu ter ido ler o ponto de chamada — e
  na primeira só porque quis registrar o falso positivo, não porque desconfiei.
  Guarda para isso não existe, e não vejo uma que não seja casador de texto,
  que é o que esta casa já recusou com número.
