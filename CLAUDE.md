# PhxSql — convenções do projeto

Motor de dados em Rust no modelo de arquivos separados do HFSQL. O código vive
em `phxsql/`. Especificação do formato em `phxsql/docs/FORMATO.md`, roteiro em
`phxsql/docs/PLANO.md`.

## Ao terminar cada rodada de trabalho: atualize o dossiê

O dossiê é a página que o Adriano usa para enxergar o projeto inteiro:

- **URL:** https://claude.ai/code/artifact/5c14044e-0dc5-4832-b015-224ab1e40033
- **Fonte:** `phxsql/docs/dossie/dossie-phxsql-0.18.html` (versionado, para que
  qualquer sessão consiga atualizá-lo)

Publique sempre **passando essa URL**, para cair na mesma página em vez de
criar outra. Instruções e as armadilhas de estilo em
`phxsql/docs/dossie/LEIA-ME.md`.

O nome muda a cada refação — era `dossie-phxsql.html`, virou `-0.15` e agora é
`-0.18` — e **só existe um por vez**: o anterior sai do repositório no mesmo
commit, para que ninguém atualize o errado. Todos os scripts aceitam o caminho
do HTML como argumento, então trocar o nome de novo não exige editá-los.

Os números do painel são **medidos, nunca estimados** — já saíram errados três
vezes: arredondamento para cima, depois 276 testes quando eram 280, depois um
rodapé inteiro parado numa versão anterior. **Nenhum número visível se digita
mais**: são **cinco** geradores, listados no `LEIA-ME.md` da pasta, e eles
escrevem o título, o selo, o painel da capa, o rodapé, os idiomas, a bancada, o
painel da replicação, os pedidos, a cobertura por área e as capturas.

E o corolário que a revisão da 0.18 pagou: **a receita de um número também
envelhece.** A do KiB de interface era uma lista de três arquivos copiada no
script; o `http.rs` passou a embutir nove, e o rodapé publicou **780 KiB**
quando a interface tinha **1.032**. Hoje a lista sai do próprio `http.rs`.
Quando um gerador depende de uma lista, a lista tem de sair do código.

O que falta no projeto está em `phxsql/docs/PENDENCIAS.md` — atualize junto com
o dossiê.

Dessa lista sai uma **segunda página**, a relação dos pedidos com o estado de
cada um:

- **URL:** https://claude.ai/code/artifact/d6c8f13c-e4a2-444e-9f19-0e047e230352
- **Fonte:** `phxsql/docs/dossie/pedidos.html`, que **não se edita** —
  `python3 phxsql/docs/dossie/pagina-dos-pedidos.py` a gera do `PENDENCIAS.md`
  e conta os três estados sozinho.

## A marca é oficial

Os arquivos estão em `phxsql/marca/`, com a especificação em
`phxsql/marca/LEIA-ME.md`. Tipografia **Exo 2**, fundo `#010418`, assinatura
*Built to store. Engineered to scale.*

A marca **manda** sobre qualquer paleta inventada. Duas adaptações já
decididas e documentadas: o corpo de texto longo não usa Exo 2, e o vermelhão
escurece para `#C63C0A` no tema claro, por contraste.

Atenção: a folha de marca afirma *ACID compliant* e *built-in replication*.
O segundo **virou verdade** — a replicação funciona, está medida com quatro
servidores, e o cluster faz eleição e promoção automática. O primeiro
**continua falso**, e continuará enquanto não houver transação: sem ela não há
o A nem o I do ACID. Não repita *ACID compliant* em documento técnico.

## Regras que não se quebram

**Zero dependências externas.** Só a `std`. Foi o que fez a compilação cruzada
para Windows funcionar de primeira e o que permite `cargo build --offline`.
JSON, CRC-32, SHA-256, HMAC e PBKDF2 são escritos aqui. Se algo parecer exigir
uma crate, primeiro pergunte — não acrescente.

**Bancada compara trabalho igual, não só pergunta igual.** Os dois erros já
cometidos aqui saíram do mesmo lugar e apontaram para lados opostos: primeiro
um `WHERE id IN (…)` contra vinte mil buscas separadas (41× a favor do outro
motor), depois um `COUNT(*)+SUM` sobre 1.250.000 linhas contra a leitura de
20.000 (5× a favor do nosso). Nenhum dos dois aparecia no número. As quatro
regras estão em `phxsql/bancada/LEIA-ME.md`.

**Criptografia se confere contra vetor oficial.** Nada de "parece certo": os
testes trazem FIPS 180-4, RFC 4231 e os vetores de PBKDF2.

**Senha nunca em texto puro.** Nem em arquivo, nem em log, nem em resposta do
protocolo. Há teste que falha se a ficha de usuário vazar o hash.

E o corolário, que o Profiler obrigou a escrever: **funcionalidade que mostra
texto cru redige ANALISANDO, nunca recortando.** Recortar depende de o pedido
estar escrito de um jeito; analisar e reserializar não. O que não se analisa
não vira texto — vira o tamanho em bytes. Se a estrutura não se lê, não há como
tapar o campo dentro dela.

**A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot excluído.
Qualquer proposta que quebre isso precisa ser discutida antes.

**Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar o
formato é barato; depois vira migração. Foi assim com o volume no ponteiro.

**Receita de fora se mede contra o nosso gargalo antes de virar plano.** Chegou
uma arquitetura completa para acelerar escrita — WAL sequencial, group commit,
MemTable, LSM. É uma boa receita **para o gargalo que ela descreve**, que é o
`fsync` do InnoDB. Medi o nosso antes de aceitar: **83,5% do tempo de uma
inserção está no `.ndx`**, e o arquivo de dados já é *append-only* e custa
16,5%. Das dez propostas, cinco já existiam aqui, duas miravam um problema que
não temos, uma quebraria a ordem de digitação, e duas eram reais. Está em
`phxsql/docs/DESEMPENHO.md`, com o medidor (`--example onde-doi`).

**Interface só se prova exercitando.** Gravar um vídeo de demonstração achou
**três defeitos em cinco minutos** que ler o código não acharia — e o pior deles
quebrava *todo salvar e todo incluir* pela tela desde que o `rownum` entrou. O
padrão dos três é o mesmo: **coluna de sistema nova quebra quem filtra pela
primeira**. Quando entrar uma peça no fim de uma lista, procure quem usa
`find(...)` onde devia usar `filter(...)`.

**Guarda nova entra pedida, não imposta.** A janela de conflito de escrita
podia recusar toda gravação sem versão — e aí todo cliente escrito antes dela
pararia de gravar de um dia para o outro, recebendo um erro que não sabe tratar.
Quem manda `"versao"` ganha a garantia; quem não manda continua como antes; a
interface web manda sempre, porque é onde existe gente e existe a janela de
minutos entre abrir a ficha e clicar em salvar. **Proteção que quebra todo
cliente antigo não é proteção, é estrago** — e o teste que trava isso é o do
comportamento *velho*, não o do novo.

**Merge de conflito marca quem MEXEU, não quem perguntou por último.** Deixar
«o meu» marcado em todas as colunas desfaria em silêncio o trabalho do outro
nas colunas que eu nem toquei — o mesmo estrago de antes, com mais cliques. O
padrão certo é por coluna: a que eu digitei fica comigo, a que só o outro mudou
fica com ele. Dois que editaram campos diferentes saem com os dois trabalhos e
sem escolher nada.

**O CSS global morde todo componente novo da tela.** `input{width:100%}` e
`label{text-transform:uppercase}` são certos para um formulário e errados
dentro de uma tabela: o rádio virou uma bolinha do tamanho da célula, e
«Blumenau» apareceu como «BLUMENAU» — que é uma **mentira sobre o dado**, porque
quem olha não sabe se está gravado assim. Nenhum dos dois aparece lendo o
código. Componente novo se abre no navegador e se olha, e é a mesma lição do
vídeo por outro caminho.

**A lista do que falta também é palpite até alguém medir.** O pedido 113 dizia
«ordene as chaves do lote antes do `.ndx`» e vinha com o alvo certo — os 83,5%
estavam mesmo lá. Só que o custo não era de **localidade**: era de reler do
arquivo e recalcular o CRC-32 da **mesma página** a cada descida da árvore. A
desordem custava 1,06×; ordenar teria comprado quase nada, e teria custado uma
garantia. Um cache de páginas de leitura comprou **2,40×**. *Medir a premissa do
item vem antes de implementar o item* — inclusive quando o item é nosso.

E o corolário: o mesmo medidor dizia «~20 toques de página por linha», citando
um `strace` de outro dia. Eram 10,86, e é por isso que a conta do CRC nunca
fechava naquele documento. **Número citado é número que não se mede** — hoje o
medidor conta os toques por dentro.

**Portão de permissão é UM só — e o campo que ele lê é o furo.** O direito por
tabela entrou no despachar, que confere o campo `"tabela"` do pedido. Duas
operações não têm esse campo: `juntar` guarda as tabelas em `a.tabela` e
`b.tabela`, e `unir` guarda numa **lista**. Sem conferência própria, bastaria
pedir a tabela negada como o lado B de uma junção. **Quando o portão passar a
olhar um campo novo, procure quem não tem esse campo** — e não espalhe o portão
por quarenta operações, porque a que alguém esquecer vira a porta dos fundos e
ninguém acha por leitura.

E o teste que mais importa numa regra de permissão nova é o do comportamento
**velho**: `sem_regra_de_tabela_nada_muda`. Regra que muda o significado da
configuração que já existe tira o direito de alguém sem ninguém ter pedido.

**Instrumentação desligada tem de custar zero — e o portão que decide isso vem
ANTES do trabalho.** O Profiler desligado cobrava 7% da carga pela rede: o ponto
de captura fazia dois `Json::analisar` do corpo inteiro, três `String` e um
mutex, e só então perguntava se estava ligado. Num lote de cinco mil linhas era
analisar meio megabyte de JSON duas vezes para jogar fora. Quando entrar um
observador novo, procure o que ele faz antes de olhar o próprio interruptor.

E o corolário sobre a **explicação** disso, que eu errei primeiro: escrevi que
«o mutex era o pior pedaço, porque serializa». Medido, o `lock` sem disputa
custa **13,2 ns** e o parse do lote custa **3.456 µs** — 262.000× mais. O mutex
nunca foi o gargalo, e neste servidor nem poderia ser: a trava global de dados
já serializa tudo, e é tomada depois e segurada por mais tempo. **Diagnóstico
plausível não é diagnóstico medido** — e o errado sobrevive melhor quando o
conserto funcionou por outro motivo.

**Teste unitário não prova queda de conexão — soquete prova.** Os dez testes do
`BULKINSERT` passavam, e a prova pelo soquete mostrou que a queda da conexão
**não soltava a reserva**. A causa não estava no servidor: era o teste, porque
`socket.makefile()` do Python segura o descritor e fechar só o soquete deixa o
fd aberto — o servidor nunca via o fim da conexão. Duas lições numa: o que
depende do sistema operacional se prova contra o sistema operacional, e um teste
que passa por engano é pior que um teste que falta.

**Configuração que não é lida mente.** `recursos.cache_paginas` estava no
`config.json`, no MANUAL e na tela desde a 0.13.0, e **nenhuma linha de código
o lia** — o campo dizia "4096 páginas do `.ndx` em memória" quando não havia
cache nenhum. Campo de configuração sem leitor é pior que campo ausente: o
ausente ninguém ajusta esperando efeito.

**Medidor com binário velho mede o passado.** `cargo build --release` não
recompila os *examples*, e a bancada chama `target/release/examples/carga`
direto: uma rodada inteira de ganhos (16,4 → 7,5 µs) ficou invisível na bancada
porque o binário dela era de antes — e a conclusão «o esquema custa 2,2×»
nasceu, com tabela e tudo, dessa diferença. Antes de medir:
`cargo build --release --examples -p phxsql-store`.

**Número digitado à mão envelhece calado.** O selo da capa do dossiê passou
**quatro lançamentos** dizendo 0.11.0 — e o script que existe justamente para
impedir isso não cobria aquele pedaço. Todo número visível ou sai de um gerador,
ou está errado e ninguém percebeu ainda.

**Texto de tela entra pela fábrica de idiomas — isso é pétreo.** Palavra do
dono: *«o agente multi linguagem deve fazer uma revisão constante para manter a
possibilidade de mudar entre português, inglês… pelo login e pela tela de
configuração. A cada nova implementação esse agente tradutor deve atualizar
strings fixas por variáveis de multi linguagem.»* A máquina existe desde a
0.17.0 — `phxsys.mensagens`, a `FABRICA_TELA` do `idiomas.rs`, as bandeiras do
login. O que faltava era o laço que **conta**: medido antes desta rodada,
11.987 linhas de interface e **16** `data-txt`. Máquina que funciona e que
ninguém usa é promessa, não garantia.

O conferidor é `crates/phxsql-server/src/conferidor.rs`, e ele roda junto dos
testes: `cargo run --example textos-fora-da-fabrica -p phxsql-server` lista
arquivo e linha de cada texto que ainda está cravado, e a **catraca** (`TETO`)
reprova quem acrescentar mais um. O número só desce — traduziu um punhado,
baixe a catraca no mesmo commit, porque catraca frouxa não segura nada. O
procedimento de acrescentar um texto está em `docs/MENSAGENS.md`.

Três armadilhas que esta frente já pagou:

- **Rótulo se traduz; dado, nunca.** É a mesma lição do «Blumenau» virando
  «BLUMENAU». No conferidor ela virou crivo: tudo o que a página **interpola**
  (`${…}`) some antes da varredura, e só sobra o que alguém digitou no fonte.
- **Texto se resolve por CHAVE, nunca por comparação da frase.** No dia em que
  alguém melhorar a redação, quem compara frase para decidir a tradução quebra
  calado — e quebra em silêncio, mostrando português.
- **Chave morta é pior que chave faltando.** O tradutor a vê na tabela, traduz
  nos seis idiomas, e nada muda na tela. Há teste para os dois lados do laço:
  chave que a tela pede e não existe, e texto que existe e ninguém pede.

E as **três mensagens que não se traduzem de propósito** continuam assim, com o
motivo escrito no `mensagens.rs`: `erro.redireciona` (o cliente recorta o
prefixo — é protocolo vestido de texto), `erro.sinal` (a `MESSAGE_TEXT` é do
dono do banco) e `erro.cancelado` (o texto já vem montado). Achou uma quarta do
mesmo naipe? Documente a decisão em vez de traduzir.

**Toda bateria de testes tem prova real e aprendizado documentado — frutífero
ou infrutífero.** Prova real é nos dois sentidos: o teste novo tem de **falhar
com o defeito reposto** e passar com o conserto (já houve teste que passava por
engano, e ele é pior que teste que falta). O aprendizado vai para o documento
da área (`DESEMPENHO.md`, `SEGURANCA.md`…), não só para a conversa — inclusive
quando a hipótese **morre**: a recusa com o número é resultado tão válido
quanto o ganho, e é o que impede a mesma ideia de voltar sem medição. E
hipótese infrutífera não encerra a bateria: **gera a próxima hipótese**, como
na caça aos 2,3× do insert, em que cinco suspeitos caíram medidos antes de o
binário velho aparecer.

## Antes de commitar

```bash
cargo fmt --all
cargo clippy --workspace --all-targets     # tem de dar zero avisos
cargo test --workspace
```

Mexeu no formato em disco? Atualize `docs/FORMATO.md` no mesmo commit.

## Cores da ação, na interface

Convenção decidida e aplicada: **verde inclui, amarelo altera, rosa marca (o
excluir que volta), vermelho exclui de vez, azul consulta.**

Sempre **contorno, nunca fundo cheio** — a lição já estava num comentário do
CSS antes de virar regra: fundo laranja com texto escuro em cima ficava
ilegível, e foi assim que o botão de excluir apareceu. O preenchimento só
acontece no `hover`, quando há intenção. No tema claro as cinco escurecem, pelo
mesmo motivo do vermelhão da marca: verde e rosa claros não passam de 4,5:1
sobre papel.

## Estilo

- Código, comentários, documentação e mensagens de commit em **português**.
- Identificadores e comentários **sem acento** (o texto de interface pode ter).
- Comentário explica **por que**, não o que — o código já diz o que.
- Mensagem de commit conta a decisão e o motivo, não a lista de arquivos.

## Branch

Trabalhe em `claude/capacidades-disponiveis-y6auxh`, em
`adrianoboller/adrianoboller`. Não abra PR sem pedido explícito.
