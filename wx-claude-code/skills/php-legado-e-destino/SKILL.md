---
name: php-legado-e-destino
description: "PHP nos dois sentidos: ler um sistema PHP legado (procedural ou OOP) extraindo regra de negócio, e converter para PHP 8.3 como destino."
metadata:
  short-description: PHP legado lido e PHP como destino da conversão
  origem: escrita para o plugin WX Claude Code em 4 de setembro de 2026
---

# PHP: legado a converter e destino da conversão

Esta skill serve dois fluxos que compartilham a mesma tabela de equivalências:

- **PHP como origem**: o cliente tem um sistema PHP (sozinho ou ao lado do WX) e quer sair dele. Você lê o PHP como lê o WLanguage: com localizador, sem inventar.
- **PHP como destino**: o cliente escolheu PHP 8.3 em `H_backend.perfil = "php"`. Você converte WINDEV, WEBDEV, WINDEV Mobile ou o próprio PHP legado para PHP moderno.

As regras do projeto valem aqui inteiras: **regra de negócio só existe com origem localizável** (`arquivo.php#linha`), **senha nunca em texto puro**, e **o que não se analisa não vira texto**.

## Parte 1 — Ler PHP legado

### Inventário antes de qualquer leitura de código

Sem inventário, você lê o arquivo errado por uma hora. A ordem é sempre:

1. **Detectar a era e o estilo.** Procure nesta ordem: `composer.json` (projeto moderno, PSR-4), `artisan` (Laravel), `bin/console` e `config/services.yaml` (Symfony), `system/core/CodeIgniter.php` (CodeIgniter), `wp-config.php` (WordPress), `index.php` na raiz com `include`/`require` (procedural clássico). Um sistema pode ter duas eras convivendo — anote as duas.
2. **Grafo de inclusão.** `require`, `require_once`, `include`, `include_once` e `spl_autoload_register` dizem a ordem real de carga. Arquivo que ninguém inclui é candidato a morto: marque, não apague.
3. **Pontos de entrada.** Todo `.php` acessível pelo servidor web é um ponto de entrada; num roteador único (`index.php` + rotas) só ele é. A diferença muda tudo na conversão.
4. **Tabelas tocadas.** Toda string SQL, todo `->table(`, todo model Eloquent/Doctrine. É o que amarra o PHP ao mesmo banco que o WX usa, se for o caso.
5. **Dependências.** `composer.json`, extensões (`ext-*`), `ini_set`, `exec`/`shell_exec`, bibliotecas copiadas para dentro do projeto sem gerenciador.

### Onde a regra de negócio se esconde no PHP

| lugar | o que procurar | por que importa |
| --- | --- | --- |
| Corpo do arquivo, fora de função | código solto entre `<?php` e HTML | no PHP procedural, a regra costuma estar aqui, não em função |
| `if` que decide preço, prazo, imposto, permissão | comparações com constantes mágicas | vira `BR-*` com o número literal preservado |
| SQL embutido | `WHERE`, `CASE`, `HAVING`, funções de agregação | metade da regra costuma estar no SQL, não no PHP |
| Trigger e procedure no banco | `DELIMITER`, `CREATE TRIGGER` em `.sql` do repositório | regra que roda sem passar pelo PHP |
| Validação de formulário | `$_POST` conferido antes de gravar | é o `VALID` do WX, e some se não for procurado |
| Cron e fila | `crontab`, `supervisor`, `queue:work` | rotina noturna é regra de negócio que ninguém lembra de citar |
| Constante e `define()` | alíquota, limite, prazo | valor que muda o resultado e não está em tabela |

### Armadilhas do PHP que mudam o resultado convertido

Cada uma destas já quebrou conversão; confira contra o comportamento observado, não contra a intuição:

- **Comparação frouxa.** `==` entre string e número, `"abc" == 0` (falso no PHP 8, verdadeiro no PHP 7). A versão do PHP legado muda a regra. Anote a versão em `DEC-*`.
- **Ponto flutuante em dinheiro.** `0.1 + 0.2 != 0.3`. Se o legado somava `float`, o total do legado pode estar errado — e o golden master vai acusar. Decida com o aprovador: preservar o erro ou corrigir (é `DEC-*`, nunca escolha silenciosa).
- **Array associativo ordenado.** A ordem de inserção importa em relatório; um `foreach` no PHP não é um `HashMap` de outra linguagem.
- **`null` e string vazia.** `empty("0")` é verdadeiro. Campo com `"0"` tratado como vazio é bug antigo que virou regra.
- **Fuso e `date_default_timezone_set`.** Data gravada em UTC e exibida em local, ou o contrário, sem conversão.
- **`mysql_*` (removido), `mysqli`, PDO.** O legado pode misturar os três; `mysql_real_escape_string` ausente é injeção, e vira `SEC-*` no destino, não uma cópia fiel.
- **Sessão como estado de negócio.** `$_SESSION['carrinho']` guardando o que devia estar em tabela.
- **Encoding.** `latin1` no banco, `utf8` na página, `utf8mb4` no destino: acento vira dado corrompido se a migração não converter.

### O que vira o quê, na matriz

| peça do PHP | id na matriz | prova |
| --- | --- | --- |
| função ou método com decisão | `BR-*` | golden master com os mesmos dados |
| consulta SQL | `QRY-*` | mesmo resultado, mesma ordem |
| formulário e listagem | `UI-*` | tela nova aceita o mesmo caminho de teclado |
| relatório em PDF/Excel | `RPT-*` | PDF comparado página a página |
| integração, webhook, cron | `INT-*` | mesma entrada, mesma saída, com idempotência |
| tabela e coluna | `DB-*` | contagem e soma por tabela após migrar |

## Parte 2 — PHP como destino

Perfil `php`: **PHP 8.3**, Laravel 11 por padrão (ou Symfony 7 quando o cliente já usa), Composer, PSR-12, `declare(strict_types=1)` em todo arquivo.

| peça de origem | vira em PHP |
| --- | --- |
| Procedure global/local (WLanguage) ou função solta (PHP legado) | método de classe de serviço, um serviço por domínio |
| Classe WLanguage | classe PHP com `readonly` onde couber; herança só a que existe |
| Análise HFSQL / esquema MySQL | migrations do Laravel; um model por arquivo, `$casts` explícitos |
| `HReadSeek*`/`HAdd`/`HModify` ou `mysql_query` | repositório por entidade, Query Builder ou Eloquent, sempre com parâmetro ligado |
| Query `.WDR` ou SQL embutido | método de repositório com SQL explícito e teste de resultado |
| Janela ou página | rota + controller + Blade (ou Inertia/Livewire se o front for reativo) |
| Relatório `.WDE` | template Blade renderizado a PDF (Dompdf ou Browsershot), comparado página a página |
| Funções de string, data, arquivo, JSON | ver a tabela de equivalência abaixo |

### Regras não negociáveis no PHP gerado

1. **Dinheiro nunca em `float`.** Inteiro de centavos, ou `brick/math` com `BigDecimal`. Coluna `DECIMAL(19,4)`.
2. **Consulta sempre parametrizada.** Nenhuma concatenação de variável em SQL, nem em `whereRaw`.
3. **`declare(strict_types=1)`** e tipos em todo parâmetro e retorno; `mixed` só com comentário dizendo por quê.
4. **Sem estado global.** Nada de `global`, `$GLOBALS` ou singleton escondido; injeção de dependência.
5. **Segredo por variável de ambiente**, lido em `config/`, nunca `env()` fora de config.
6. **Transação explícita** em toda escrita com mais de uma tabela, com evento de domínio na mesma transação.

### Equivalência WLanguage → PHP (as mais usadas)

Consulte o corpus 12k por tema (`query_wlanguage_help.py --group`) para a semântica exata antes de confiar na linha; a coluna «cuidado» é onde a tradução ingênua erra.

| WLanguage | PHP 8.3 | cuidado |
| --- | --- | --- |
| `Left(s,n)` / `Right(s,n)` | `substr($s,0,$n)` / `substr($s,-$n)` | acento: use `mb_substr` |
| `Middle(s,i,n)` | `mb_substr($s,$i-1,$n)` | WLanguage começa em 1, PHP em 0 |
| `Length(s)` | `mb_strlen($s)` | `strlen` conta bytes |
| `NoSpace(s)` | `trim($s)` | `NoSpace` remove todos os espaços conforme a constante |
| `Upper`/`Lower` | `mb_strtoupper`/`mb_strtolower` | sem `mb_`, acento não muda |
| `Position(s,b)` | `mb_strpos($s,$b)` | retorna `false`, não 0, quando não acha |
| `Replace(s,a,b)` | `str_replace($a,$b,$s)` | ordem dos argumentos é inversa |
| `StringCount(s,b)` | `substr_count($s,$b)` | |
| `Val(s)` / `NumToString(n,f)` | `(int)`/`(float)` / `number_format` | `Val` aceita vírgula decimal conforme o idioma |
| `DateSys()` / `TimeSys()` | `new DateTimeImmutable()` | WX guarda `AAAAMMDD` e `HHMMSSCC` como string |
| `DateDifference(d1,d2)` | `$d1->diff($d2)->days` | sinal e inclusão do dia final |
| `DateValid(d)` | `checkdate($m,$d,$a)` | |
| `Round(n,c)` / `Int(n)` | `round($n,$c)` / `intdiv` ou `(int)` | meio-para-cima do WX ≠ meio-para-par |
| `fFileExist` / `fLoadText` / `fSaveText` | `file_exists` / `file_get_contents` / `file_put_contents` | trate `false` de retorno |
| `fSep`/`fExtractPath` | `DIRECTORY_SEPARATOR` / `pathinfo` | |
| `JSONToVariant` / `VariantToJSON` | `json_decode($s,true)` / `json_encode` | `JSON_THROW_ON_ERROR` e `JSON_UNESCAPED_UNICODE` |
| `HExecuteQuery`/`HReadSeekFirst` | repositório + Query Builder | leitura por chave vira `where()->first()` |
| `HAdd`/`HModify`/`HDelete` | `save()`/`update()`/`delete()` | dentro de transação |
| `HFilter` | escopo de consulta (`scope`) | filtro do WX é estado, não parâmetro |
| `Info`/`Error`/`YesNo` | resposta HTTP + toast; `confirm` no front | mensagem exata do legado (F9) |
| `Trace` | `Log::debug` | nunca com dado pessoal |
| `EmailSendMessage` | `Mail::send` | |
| `HTTPRequest` | `Http::` (Guzzle) | timeout e retry explícitos |
| `Serialize`/`Deserialize` | `serialize` só interno; JSON para tráfego | nunca `unserialize` de entrada externa |

### Estrutura do projeto PHP gerado

Segue o esqueleto de ERP do plugin (L6) com nomes PHP: `app/<Modulo>/` (Actions, Models, Repositories, Http), `database/migrations`, `tests/{Unit,Feature}` espelhando `tests/{unit,domain,integration,contracts,security,migration,e2e}`, `routes/`, `config/`. O `composer.json` fixa versões; `phpstan` nível 8 e `pint` no CI.

## Quando NÃO usar esta skill

- Semântica exata de uma função WLanguage: use o corpus 12k por tema, não a tabela acima.
- Regra de negócio: ela vem da matriz e do legado, nunca desta skill.
- Qualidade de tela: `qualidade-erp.md` e o Impeccable.
