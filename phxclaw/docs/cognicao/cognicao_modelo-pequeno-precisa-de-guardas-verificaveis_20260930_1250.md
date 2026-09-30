# Agente com modelo pequeno: a conclusão se verifica, não se aceita

**Estado:** FRUTÍFERO

**Evidência:** `6787eb8`; `894f2c1`

## O que aconteceu

Primeiras execuções reais do agente com Ollama local (mesmo objetivo: buscar a versão do
Rust e gravar `rust.xlsx`):

- 1,5B: buscou de verdade (achou 1.98.1), depois entrou em laço — `read_file("/work/...")`
  negado 4 vezes e a MESMA busca 8 vezes, até o DuckDuckGo bloquear por robô. 33.621 tokens.
- 3B: achou a versão sem laço (2.924 tokens), mas terminou NARRANDO "I will create rust.xlsx".
- 3B com `final_answer`: chamou `final_answer` dizendo que criou a planilha sem ter usado
  nenhuma ferramenta.
- Servidor + 3B pedindo site: depois de 3 recusas, uma resposta em texto saiu `completed`
  sem o arquivo.

## O que eu concluí primeiro, e estava errado

Que bastava trocar para um modelo maior. O 3B resolveu o laço, mas trouxe dois defeitos
novos (narrar e alucinar conclusão). O que resolveu foi o motor não aceitar a palavra do
modelo sobre o próprio trabalho.

## O que a medição disse

Com as quatro guardas (caminho `/work` = relativo; 3ª chamada idêntica não roda; fim é
ferramenta; conclusão exige os arquivos citados no objetivo), o 3B foi de ponta a ponta:
busca real → `rust.xlsx` gravado com hash. O conteúdo da planilha saiu errado (versão
ausente): isso é o limite do modelo, e o status agora não esconde nada.

## A regra

Todo "terminei" de agente passa por verificação objetiva do que foi pedido; o status final
é do motor, não do modelo.

## Como está guardado hoje

`concluir()` e `arquivos_pedidos()` em `crates/phxclaw-agent/src/motor.rs`, com os testes
`final_answer_sem_o_arquivo_pedido_e_recusado`, `resposta_sem_o_arquivo_pedido_termina_falha_e_nao_concluida`,
`terceira_chamada_identica_nao_roda` e `narrar_a_acao_nao_encerra_quando_o_fim_e_ferramenta`.
Buraco: a verificação só enxerga NOMES de arquivo citados no objetivo; "a planilha tem a
versão certa" não se verifica.
