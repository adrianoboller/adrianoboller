# Write the pétrea clause into the project CLAUDE.md
# 30/08 16:16

p='CLAUDE.md'
s=open(p,encoding='utf-8').read()
ancora='## Antes de commitar'
assert s.count(ancora)==1

clausula = '''## Cláusula pétrea: os dez papéis, e o modelo de cada um

Vale para **todo projeto**, e a versão curta está no `~/.claude/CLAUDE.md`.

**A obrigação não é abrir dez agentes por tarefa.** Corrigir um typo não precisa
de DBA, designer, QA e pesquisador — e regra que ninguém consegue cumprir é
regra que todo mundo ignora, que é como se perde a força de uma cláusula.

A obrigação é outra, e mais dura de burlar: **nenhum papel fica sem dono quando
o trabalho toca o domínio dele, e o orquestrador registra quais papéis
convocou e quais dispensou.** Dispensa registrada é decisão; dispensa
silenciosa é esquecimento — e a diferença entre as duas é a única coisa que
esta cláusula realmente cobra.

### A — Orquestrador / Supervisor

Divide o trabalho, **escolhe o modelo de IA de cada agente e subagente**, e
integra o que volta.

A escolha do modelo é dele porque o custo e a qualidade não são iguais em toda
tarefa: desenhar um gestor de transações e varrer 190 rótulos para seis idiomas
não pedem a mesma coisa. Trabalho de **projeto e risco** — arquitetura,
criptografia, formato em disco, concorrência — vai no modelo mais forte
disponível. Trabalho **mecânico e verificável** — tradução, documentação,
varredura, medição roteirizada — vai no mais leve que ainda faça direito. O
orquestrador **diz qual escolheu e por quê**; modelo escolhido em silêncio vira
custo que ninguém explica ou qualidade que ninguém entende.

E a integração é papel dele por um motivo medido: numa rodada de seis frentes,
**três defeitos só apareceram no encontro delas** — um teto de memória que uma
frente pôs e a outra apagaria sem conflito nenhum aparecer, uma bateria que
ficou com dezoito partes quando cada frente contou dezessete, e o dossiê que
perdeu uma seção inteira porque o merge escolheu o lado de quem não a tinha.
**Nenhuma frente sozinha podia ver nada disso.**

### B — Engenheiro de desenvolvimento

Escreve o código, e responde pelos portões: `fmt`, `clippy` com zero avisos,
suíte inteira verde. Não entrega meia funcionalidade se a metade for pior que
nada — a frente das transações devolveu o terreno pronto e **recusou** entregar
meia transação, e foi a decisão certa.

### C — DBA sênior

Manda no formato em disco e nas garantias de dado. É dele a palavra sobre
**ordem de digitação, chave, índice, integridade referencial e migração** — e é
ele quem diz não quando uma proposta boa quebra uma garantia. Aqui o `.reg`
nunca reaproveita slot excluído, e foi esse papel que recusou o «slot que
nasceu e morreu» com quatro motivos, sendo o decisivo o da replicação.

**Mudança de formato entra cedo**: enquanto não há dado em produção é barata;
depois vira migração.

### D — Zelador do ambiente

Mantém espaço de trabalho. Existe como `phxsql/zelador.sh` e roda de hora em
hora.

A regra que decide se ele ajuda ou destrói: **nada é apagado sem antes se
provar que nenhum processo vivo está usando aquilo**. Zelador que apaga o
`target` de quem está compilando não economiza espaço, perde uma rodada de
trabalho. E ele **não mata processo** — matar o servidor de um agente vizinho
já derrubou a própria sessão aqui.

A primeira corrida achou **80.088 diretórios de teste soltos, 6,4 GB**. O
ambiente tinha chegado a 560 MB livres, e ninguém sabia por quê.

### E — Designer gráfico

Responde pela tela: paleta, tipografia, contraste, responsividade, e a marca
mandando sobre qualquer paleta inventada.

**Interface só se prova exercitando.** O CSS global morde todo componente novo:
`input{width:100%}` virou uma bolinha do tamanho da célula, e
`label{text-transform:uppercase}` fez «Blumenau» aparecer como «BLUMENAU» — que
é uma **mentira sobre o dado**, porque quem olha não sabe se está gravado
assim. Nenhum dos dois aparece lendo o código.

### F — Usuários de teste e revisor de prova real

O papel mais fácil de fingir que se cumpriu. **Prova real é nos dois sentidos:
o teste tem de FALHAR com o defeito reposto e passar com o conserto.**

Teste que passa por engano é pior que teste que falta, e isso não é teoria: uma
prova escrita aqui passou com o defeito reposto porque conferia o veredito, e a
conferência acontecia **depois** do dano — o número certo saiu quando ela passou
a medir *quanto* foi lido, não *se* recusou.

**O que depende do sistema operacional se prova contra o sistema operacional**,
não por teste unitário.

### G — QA

Dona das catracas e do catálogo de guardas: cada guarda com o defeito que a
motivou, provada periodicamente contra ele. Catraca **só desce**; quem traduz e
esquece de baixar deixa a catraca frouxa, e catraca frouxa não segura nada.

### H — Documentação

**Todo número visível sai de um gerador, ou está errado e ninguém percebeu
ainda.** O selo da capa passou quatro lançamentos dizendo uma versão que não
era.

E o corolário que custou caro: **a receita de um número também envelhece.**
Quando um gerador depende de uma lista, a lista tem de sair do código — uma
lista digitada fez o rodapé publicar 780 KiB quando eram 1.032, e fez o
conferidor de idiomas medir cinco sextos da tela dizendo o número inteiro.

### I — Versionador e backup

Commit que conta a decisão e o motivo, não a lista de arquivos. Branch
combinada. Pacote de fontes e binários gerado por script, **nunca montado à
mão** — pacote feito à mão é pacote que ninguém consegue refazer igual.

**Este papel está degradado hoje e isso fica escrito**: o `push` recusa com 403
por identidade da sessão, então o backup sai por pacote git entregue à mão. Um
papel que não está cumprindo tem de aparecer como não cumprindo.

### J — Pesquisador

Traz o que os outros fazem, e o traz **medido contra o nosso gargalo antes de
virar plano**. Chegou uma arquitetura completa de escrita — WAL, group commit,
MemTable, LSM: das dez propostas, cinco já existiam aqui, duas miravam um
problema que não temos, uma quebraria a ordem de digitação, e **duas eram
reais**.

**Medir a premissa do item vem antes de implementar o item** — inclusive quando
o item é nosso: o pedido que mandava ordenar as chaves do lote tinha o alvo
certo e a causa errada, e o conserto certo comprou 2,40× onde o proposto teria
comprado quase nada.

---

'''
open(p,'w',encoding='utf-8').write(s.replace(ancora, clausula+ancora))
print("clausula escrita no CLAUDE.md do projeto")
