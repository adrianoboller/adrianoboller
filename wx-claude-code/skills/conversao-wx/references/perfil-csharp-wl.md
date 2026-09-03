# Perfil C# com WL_C#: WLanguage para .NET com as mesmas funções

**WL_C#** (Bernard Sobra, https://bernardsobra.github.io/WL-web/) é uma
biblioteca C# gratuita que reproduz funções do WLanguage com o nome francês
original e o mesmo comportamento: `DateVersChaîne`, `ChaîneOccurrence`,
`TableauAjoute`, `fRepEnCours`, `JSONVersVariant`, `MarkdownVersPDF`. A
página declara mais de 480 funções, 43 tipos avançados e mais de 1.900 testes
automatizados. O código-fonte não é publicado; a distribuição é o `WL.dll`
da release.

Para o plugin ela importa por um motivo: numa conversão para C#, a maior
parte das funções padrão do WLanguage (strings, datas, arquivos, conversões,
tabelas em memória, JSON) vira **`equivalente`** em vez de **`adaptar`**, e a
tradução dessas procedures fica quase mecânica.

## O que o plugin embute

- `resources/wl-csharp/funcoes.json`: 261 nomes de função lidos do metadado
  do `WL.dll` 1.0 (SHA-256 `2ad2acdfee5c9a9d…`, 381.952 bytes). É um índice
  de existência, não a documentação: nomes com acento podem vir truncados e a
  lista é menor que as 480 declaradas porque a leitura por strings não
  enxerga tudo. Serve para o especialista responder «existe em WL_C#?» sem
  adivinhar.
- Este documento e a linha do perfil em `perfis-de-destino.md`.

O `WL.dll` **não** vem no plugin: é obtido pelo usuário na release oficial
(https://github.com/BernardSobra/WL-web/releases/tag/1.0) e conferido pelo
hash acima. A licença de redistribuição não está publicada; o site diz
«100 % gratuit», e o plugin trata isso como uso livre pelo usuário, não como
autorização para empacotar.

## Como o especialista usa

Quando `conversion.config.json` tem `target.language` começando por `C#`
e `target.frameworks` contém `WL_C#`, o `wl-standard-functions-specialist`:

1. Consulta a semântica no Help, como sempre (`--group 01-04-04`).
2. Confere se o nome francês existe em `funcoes.json`. O Help em inglês dá o
   nome inglês (`DateToString`); o nome francês correspondente vem da
   própria página do Help, que traz os dois.
3. Marca:
   - `equivalente` quando existe em WL_C# e a semântica bate (assinatura,
     retorno, tratamento de vazio e de erro);
   - `adaptar` quando existe mas há diferença documentada (por exemplo,
     cultura de formatação de data, que no .NET depende do `CultureInfo`);
   - `substituir` quando não existe (HFSQL, controles de tela, impressão,
     comunicação) e o destino é a API .NET correspondente.
4. Registra a evidência: página do Help com hash, e a linha de
   `funcoes.json`.

## O que WL_C# não cobre, e o plugin diz isso

- **HFSQL**: `HReadSeekFirst`, `HAdd`, `HModify`, transações. Vão para o
  banco de destino via Entity Framework, Dapper ou SQL direto, com o
  `data-migration-architect`.
- **Telas e controles**: janelas, tabelas, combos. Vão para Blazor, WinForms,
  WPF ou React, com o `ui-flow-analyst` e o `DESIGN.md`.
- **Comunicação** (e-mail, HTTP, soquete): API .NET, com o
  `wl-communication-specialist`.
- **Relatórios e impressão**: biblioteca de relatório do destino.

## Onde entra no fluxo

- **Questionário, letra H**: «C# (.NET 8) + WL_C#» é uma das opções, com a
  orientação de `perfis-de-destino.md`.
- **G3**: o ADR de linguagem cita este perfil e a decisão de baixar o
  `WL.dll` com hash conferido.
- **G4**: o piloto mede quantas funções caíram em `equivalente`; esse número
  entra no resumo da sprint e diz se o perfil valeu.

## Exemplo

WLanguage:

```text
sData is string = DateToString(Today(), "DD/MM/YYYY")
nPos is int = Position(sNome, " ")
```

C# com WL_C#:

```csharp
string sData = DateVersChaîne(DateDuJour(), "JJ/MM/AAAA");
int nPos = Position(sNome, " ");
```

Os nomes são os franceses porque a biblioteca segue a versão francesa do
WLanguage; o Help em inglês mostra ambos, e é por isso que o especialista
consulta o Help antes de olhar a lista.
