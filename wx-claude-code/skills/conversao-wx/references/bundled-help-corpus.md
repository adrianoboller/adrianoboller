# Corpus WLanguage Help empacotado

O plugin usa um snapshot local da documentação WLanguage apenas como evidência auxiliar e pesquisável. Ele não substitui a versão de Help declarada pelo projeto nem transforma conteúdo documental em especificação executável.

## Identidade e localização

- Caminho fixo: `${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/resources/Help_WL_12k_Json.zip`
- SHA-256 distribuído e exigido: `a95ed5536549ecc39fb1163415042d6597c8913e5edbfdb531cba756546a82a2`
- SHA-256 do anexo recebido: `a6b42f59796ccf51298712aff00c043a9be2c404ce761a99720ea31b91ca6b93`
- Raiz interna: `Help_WL_12k_Json/`
- Tamanho descomprimido declarado: `115844631` bytes
- Membros: 12.039 (1 diretório, 12.038 arquivos)
- JSONs: 12.037 (1 índice e 12.036 arquivos de página)
- Arquivo adicional: `Help_WL_12k_Json/progresso.ini`

Não declare este snapshot como completo ou íntegro. A identidade binária é verificável, mas a confiança de conteúdo é **DEGRADED/CONDITIONAL** pelos defeitos conhecidos abaixo.

## Sanitização de segurança

O anexo recebido continha 15 blocos PEM de chaves privadas demonstrativas em
duas páginas sobre JWT. A edição distribuída substitui somente esses blocos por
um marcador de redação, mantendo as páginas e os metadados. O script de sanitização (`sanitize_help_corpus.py`, mantido fora desta
distribuição) fixa o hash do anexo, os dois membros, seus
hashes e a quantidade de blocos; qualquer divergência interrompe a geração.
O verificador também rejeita qualquer bloco PEM de chave privada remanescente.

## Defeitos conhecidos e quarentena

- `Help_WL_12k_Json/01-04-01_00655__emailgetall_function__1000018727.json` contém exatamente 23.627 bytes NUL, não é JSON válido e tem SHA-256 `d95886e1dc971804e4fe98c784504c54665c5aa4a4adcc4de90e4f58e54e5148`. O utilitário tolera e põe em quarentena somente essa combinação de membro, conteúdo e hash dentro do ZIP oficial fixado; qualquer outro JSON inválido falha de forma controlada.
- Restam 12.035 páginas JSON válidas para pesquisa.
- A sequência de nomes tem a lacuna `02-03-01/00223`.
- O índice de grupos soma 12.037 páginas, enquanto existem 12.036 arquivos físicos de página.
- Há 609 identificadores lógicos repetidos, correspondendo a 613 páginas além da primeira ocorrência de cada ID. Isso não é colisão byte a byte; resultados continuam separados por `member` e `member_sha256`.
- `progresso.ini` é internamente inconsistente: `processadas=7077`, `falhas=1`, `restantes=0`, `ultima_posicao=12037` e `total_do_mapa=12037`.

O `--verify` deve continuar retornando sucesso operacional (código 0) para o snapshot oficial, mas com `status` igual a `DEGRADED/CONDITIONAL`, a lista `quarantined_members`, as lacunas, os contadores de duplicidade lógica e as inconsistências de índice/progresso. Falhas de identidade, estrutura ou segurança retornam código 2.

## Uso somente leitura

```bash
python3 ${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py --verify
python3 ${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py \
  --query "HOpenAnalysis" --version 2026 --platform WINDEV --limit 5
```

Para uma auditoria antes de o recurso ser copiado ao caminho fixo, o override exige tanto o caminho explícito quanto o hash esperado:

```bash
python3 ${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/query_wlanguage_help.py \
  --verify \
  --corpus /caminho/controlado/Help_WL_12k_Json.zip \
  --expected-sha256 a95ed5536549ecc39fb1163415042d6597c8913e5edbfdb531cba756546a82a2
```

O utilitário usa apenas a biblioteca padrão, mantém o ZIP fechado para escrita, nunca extrai membros, nunca executa conteúdo, nunca importa código do corpus e não acessa a rede. Antes de consultar, ele valida SHA-256, contagens, layout, nomes portáteis, colisões de caminhos, links simbólicos, criptografia, métodos e razões de compressão, limites de membro/total, UTF-8/Unicode, JSON, URLs `https://help.windev.com`, índice, progresso e IDs lógicos.

## Busca e saída limitada

`--query` pode ser repetido; todos os termos normalizados precisam ocorrer. O ranking determinístico pondera `nome`, `nome_curto`, títulos, trilha, sintaxes, descrição e códigos. Empates são ordenados por título, identificador e nome do membro. `--version` e `--platform` aplicam filtros exatos normalizados, e `--limit` aceita de 1 a 50.

Cada resultado expõe somente metadados limitados, `member`, `id`, versões, plataformas, URL HTTPS validada, SHA-256 do membro, campo correspondente e `excerpt` de no máximo 400 caracteres. O documento completo nunca é emitido. A saída também informa `verification_elapsed_ms` e `search_elapsed_ms`; o tempo de busca não inclui a auditoria obrigatória anterior.
