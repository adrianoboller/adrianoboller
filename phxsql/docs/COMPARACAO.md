# O que os motores maduros têm, e o que trouxemos

Revisão feita contra o **help embutido** do MySQL(R) 8.0.46 rodando na própria
máquina (705 tópicos em 53 categorias) e do MariaDB(R) 10.11 (833 tópicos em 51
categorias, lidos do `fill_help_tables.sql` do pacote). Medido, não de memória.

O objetivo não era copiar superfície — 79 tabelas de `information_schema` e 632
variáveis de sistema não fazem um motor melhor. Era achar **o que um operador
alcança lá e não alcançava aqui**.

## O que entrou

| Lá | Aqui | Por que valia |
|---|---|---|
| 5.330 códigos de erro com `SQLSTATE` | `codigo`, `nome`, `classe`, `repetir` na resposta | sem código, integrar exige comparar **texto** — e melhorar a redação de uma mensagem quebra o cliente sem ninguém perceber |
| `SHOW PROCESSLIST` | `sessoes` | o servidor sabia **contar** conexões e não sabia dizer **quem** eram |
| `KILL [CONNECTION]` | `encerrar_sessao` | quando algo prende a trava, faltava a segunda metade: como solto |
| `SHOW QUERY_RESPONSE_TIME` (MariaDB(R)) | histograma e percentis em `estatisticas` | a média escondia a cauda |
| registro de consulta lenta | `mais_lentas`, sem precisar ligar nada | o log já tinha o dado; faltava a pergunta |
| `TABLE_STATISTICS` / `USER_STATISTICS` (MariaDB(R)) | `por_tabela`, `por_usuario`, `por_erro` | o log sabia *o quê* e não *sobre o quê* |
| `CHECKSUM TABLE` | `checksum` | comparar réplica com origem sem transportar as duas |
| `SHOW STATUS LIKE 'Uptime'` | `no_ar_s` no `ping` | um servidor reiniciado de madrugada parece igual a um que nunca caiu |

### A regra dos códigos

O número só vale alguma coisa sob duas condições, e as duas valem aqui:
**número nunca muda, e número aposentado nunca volta.** Trocar o significado de
um código é pior do que não ter código, porque o cliente antigo continua
tratando pelo sentido velho. Há teste que falha se um código publicado mudar.

As faixas agrupam por família — 1000 formato, 2000 esquema, 3000 dado, 4000
acesso, 5000 sistema — e a `classe` é **derivada** da faixa, para as duas não
poderem divergir.

O que ainda é grosseiro, e vale dizer: o código é por **variante de erro**, não
por situação. `ESQUEMA_INVALIDO` cobre desde config errado até chave de junção
de famílias diferentes. Distinguir cada situação é o passo seguinte natural; o
que existe hoje já resolve o caso que importa — dá para ramificar em
«duplicado» contra «não encontrado» contra «acesso negado» sem olhar texto.

## O que ficou de fora, e por quê

**`OPTIMIZE TABLE` (compactação).** Aqui ele esbarra numa regra do projeto: o
`.reg` **nunca reaproveita slot excluído**, e a ordem de digitação é a garantia
que o TopSpeed(R) não dava. Compactar significa reescrever `rowid`, e `rowid` é
endereço — quem guardou um passa a apontar para outra linha. Uma tabela com
muitas exclusões cresce e não encolhe, e isso é hoje uma **consequência aceita**
da garantia, não um esquecimento. Mudar exige a sua decisão, não a minha.

**`ANALYZE TABLE` (estatísticas para o planejador).** Não há planejador: quem
escolhe o índice é quem escreve a operação. Estatística sem consumidor é
arquivo para manter atualizado sem ninguém ler.

**`EXPLAIN`.** Faz sentido **depois** da camada SQL. Antes dela, o equivalente
honesto seria «esta junção vai ler N linhas de A e M de B» — e isso as
estatísticas já contam depois do fato.

**`information_schema` com 79 tabelas.** O catálogo daqui são `sistabelas` e
`siscolunas`, e eles cobrem o que existe. Tabela de catálogo para recurso que
não existe seria promessa em forma de esquema.

**Transações, `SAVEPOINT`, `XA`, tabelas temporais, replicação de verdade.**
São recursos, não superfície de operação — já estão no roteiro com o que falta
de cada um.

## As duas coisas que a comparação achou por acidente

Ligar o histograma na primeira vez mostrou **p95 de 3,3 s**. Todos os acessos
lentos eram `login`: o PBKDF2 de 210.000 voltas, lento **de propósito**. O
número estava certo e a leitura anterior — «ms médio» no painel — é que era
inútil, porque a média de login com ping não descreve nem um nem outro.

E o campo `codigo` no log revelou que as linhas antigas ficam com zero. Zero
não é um erro: é uma linha gravada antes de o campo existir. A tela diz isso
com todas as letras em vez de mostrar «código 0», que pareceria um erro novo.

---

MySQL, MariaDB, TopSpeed e Clarion são marcas dos seus respectivos donos,
citadas aqui por referência técnica.
