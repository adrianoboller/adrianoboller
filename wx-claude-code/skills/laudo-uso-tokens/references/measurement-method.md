# Método de medição

Usar este método somente com fontes autorizadas e em operações de leitura. Manter a unidade de análise explícita: item de contexto, ferramenta, servidor MCP ou sessão. Nunca misturar unidades ou sessões sem indicar a agregação.

## Hierarquia de fontes

Preferir, nesta ordem:

1. campos de uso, inventários ou diagnósticos produzidos pelo próprio runtime;
2. registros primários estruturados e configurações efetivamente carregadas;
3. arquivos de projeto e de configuração autorizados;
4. contagem local com o tokenizador exato do modelo, quando modelo e tokenizador forem conhecidos;
5. estimativa reproduzível com premissa declarada.

Configuração não prova ativação ou uso. Presença em arquivo não prova pré-carregamento. Menção em texto não prova chamada de ferramenta. Data de modificação não prova data da sessão, salvo documentação explícita da fonte.

## Estados de evidência

| Estado | Quando usar | Como relatar |
| --- | --- | --- |
| `MEDIDO` | O valor vem diretamente de um campo primário autorizado ou de uma contagem determinística com tokenizador exato confirmado. | Fonte, caminho ou campo, unidade e escopo. |
| `ESTIMADO` | O valor resulta de dados parciais ou de uma premissa declarada. | Entradas, premissa, fórmula, intervalo ou incerteza e limitação. |
| `INDISPONÍVEL` | Falta fonte, autorização, campo, legibilidade ou comparabilidade. | Motivo concreto e a menor informação necessária para medir. |

Não misturar valores `MEDIDO` e `ESTIMADO` num total sem separar as parcelas. Não transformar `INDISPONÍVEL` em zero.

## Contagem de tokens de contexto

Para cada item, registrar o que foi contado: conteúdo integral, apenas frontmatter, apenas descrição ou outra parcela realmente carregada.

- Usar `MEDIDO` quando houver contagem do runtime ou tokenizador exato confirmado para o modelo.
- Na ausência disso, usar `ESTIMADO` com uma única regra reproduzível aplicada a todos os itens, por exemplo `ceil(caracteres Unicode / 4)`. Informar que idioma, código, JSON e tokenizador podem alterar o valor real.
- Como o usuário solicitou a estimativa nesta auditoria, essa aproximação pode ser usada sem uma confirmação adicional, mas nunca rotulada como medição.
- Contar bytes, palavras e linhas separadamente se forem úteis para auditoria, sem chamá-los de tokens.
- Para duplicação exata, usar hash ou comparação de conteúdo e registrar `MEDIDO`. Para sobreposição parcial, informar o algoritmo ou tratar a semelhança semântica como hipótese; estimar tokens duplicados apenas com método explícito.

Somar o conjunto pré-carregado apenas quando houver evidência de quais itens entram no contexto inicial. Manter conteúdo sob demanda, como o corpo de uma skill não acionada, fora do `PRELOAD`.

Calcular `PRELOAD/CONTEXT % = tokens de PRELOAD / context window × 100` somente quando:

- o numerador tiver escopo documentado e estado `MEDIDO` ou `ESTIMADO` explícito; e
- a capacidade da context window vier de uma fonte acessível e aplicável ao modelo efetivamente usado.

Identificar separadamente o estado do numerador e do denominador. Se a capacidade não tiver fonte acessível, omitir o número e relatar `context window: INDISPONÍVEL`; não adivinhar pelo nome ou alias do modelo.

## Inventário de ferramentas e MCPs

- Contar ferramentas de sistema/runtime a partir do registro efetivamente exposto ao agente.
- Contar MCPs configurados a partir das configurações autorizadas e MCPs ativos a partir de status/runtime; não equiparar os dois.
- Contar ferramentas por servidor MCP e o total, separando ferramentas nativas de ferramentas MCP.
- Medir ou estimar separadamente nome, descrição e schema quando forem expostos; deixar claro qual parcela compõe o contexto.
- Classificar um MCP como `ocioso na amostra` somente se estiver configurado ou ativo e não tiver chamada explícita nas sessões analisadas. Não generalizar para fora da amostra.
- Tratar descrição com pelo menos 200 tokens como `grande` para triagem, deixando esse limiar visível. Ordenar também todas as descrições por tokens para o leitor poder avaliar o limiar.

## Descoberta de CLAUDE.md, skills e agentes

Inventariar somente fontes autorizadas e distinguir:

- `CLAUDE.md` do projeto;
- `CLAUDE.md` nos diretórios pais até a raiz autorizada;
- instruções globais/de usuário acessíveis;
- arquivos importados por essas instruções, seguindo apenas imports explícitos e autorizados;
- metadata/descrição de skills e agentes potencialmente pré-carregada; e
- corpo completo carregado sob demanda ou explicitamente na sessão.

Registrar origem, escopo, relação de importação, parcela carregada e tokens por item. Sinalizar qualquer `CLAUDE.md` acima de aproximadamente 5.000 tokens e qualquer conjunto de instruções pré-carregado acima de aproximadamente 10.000 tokens. Esses são limiares de triagem, não limites oficiais.

## Seleção das sessões

1. Listar somente sessões da fonte autorizada que tenham identificador e data/hora confiável.
2. Ordenar pela data/hora registrada pela própria fonte, da mais recente para a mais antiga.
3. Analisar as 10 primeiras quando houver pelo menos 10; caso contrário, analisar todas as elegíveis e declarar o tamanho da amostra.
4. Informar o número encontrado, o número analisado e os motivos de exclusão.
5. Se não houver data/hora confiável, marcar a seleção recente como `INDISPONÍVEL`; não escolher pela data do arquivo sem documentação da fonte.

Usar um identificador anonimizado estável no relatório, como `S01` a `S10`, e não reproduzir títulos, prompts ou caminhos sensíveis.

## Métricas de sessão

Usar somente campos correspondentes na fonte:

- `input`: tokens de entrada;
- `output`: tokens de saída;
- `cache creation`: tokens gravados/criados no cache;
- `cache read`: tokens lidos do cache;
- `duração`: campo primário ou diferença entre timestamps documentados, mostrando a fórmula;
- `turnos`: campo primário ou regra declarada e consistente, sem contar silenciosamente eventos de ferramenta como turnos;
- `modelo`, `effort`, configuração e auto-switch: somente valores registrados pela fonte aplicável à sessão.

Não recompor input a partir de total quando a semântica dos campos for desconhecida. Se a fonte expuser unidades ou nomes diferentes, manter os nomes originais e documentar o mapeamento.

## Custo e ganho

- Relatar custo monetário como `MEDIDO` somente se a fonte o registrar diretamente.
- Calcular custo como `ESTIMADO` somente com tabela de preços acessível e aplicável a modelo, período, região/unidade e categorias de tokens medidos. Mostrar fórmula e moeda.
- Sem preço aplicável, usar `custo: INDISPONÍVEL`; tokens ainda podem ser relatados.
- Estimar ganho por sessão com uma linha de base comparável e hipótese explícita: `tokens da linha de base − tokens após a mudança` ou `tokens atribuíveis ao alvo × fração removível`.
- Usar intervalo quando a fração removível for incerta. Não reivindicar economia de cache como se todo `cache read` fosse cobrança ou desperdício; respeitar a semântica e o preço da fonte.

## Padrões e atribuição

Relatar como observação somente o que aparece em registros autorizados, por exemplo:

- contexto ou arquivo grande reaberto repetidamente;
- resultados de ferramenta volumosos ou repetidos;
- falha, retry, correção ou retrabalho registrado;
- cache creation alto com pouco cache read comparável;
- contexto estático reenviado sem evidência de reaproveitamento;
- ferramenta, skill ou MCP carregado sem chamada na amostra;
- respostas sistematicamente longas em relação à tarefa, quando a relação puder ser demonstrada;
- troca de modelo/effort ou auto-switch associada a repetição, apenas quando registrada.

Chamar causa de `hipótese` salvo quando a fonte estabelecer relação causal. Informar a frequência como `n/N sessões` e, quando útil, `n` eventos. Uma amostra de até 10 sessões não sustenta generalização sobre todo o projeto.

## Completude mínima

Para cada métrica, incluir estado, valor ou motivo, unidade, fonte/campo, escopo e limitação. Para cada achado, incluir frequência, evidência, distinção entre observação e hipótese, impacto e o que seria necessário para confirmar o efeito.
