# Document table rights in USUARIOS.md
# 29/08 00:33

import pathlib
p = pathlib.Path("docs/USUARIOS.md")
s = p.read_text()

alvo = '''### Três regras que decidem tudo

1. **Nega por omissão.** Atividade que não aparece na base vale `false`.
2. **A base listada manda.** Se `"Z"` está lá, vale o que está em `"Z"` — o
   `"*"` não completa o que faltou. Uma base listada vazia (`"W": {}`) nega tudo.
3. **Sem a base e sem `"*"`, nega tudo.**

Operação desconhecida exige `administrar` — o padrão é negar, não deixar passar.'''
novo = '''### Três regras que decidem tudo

1. **Nega por omissão.** Atividade que não aparece na base vale `false`.
2. **A base listada manda.** Se `"Z"` está lá, vale o que está em `"Z"` — o
   `"*"` não completa o que faltou. Uma base listada vazia (`"W": {}`) nega tudo.
3. **Sem a base e sem `"*"`, nega tudo.**

Operação desconhecida exige `administrar` — o padrão é negar, não deixar passar.

## O direito no nível da tabela

Até a 0.17.0 a permissão parava na base: quem lia a base lia **todas** as
tabelas dela. A folha de pagamento e a tabela de clientes moram no mesmo banco
porque o negócio é um só, e o direito de ler as duas não é o mesmo direito.

Dentro do objeto da base, `"tabelas"` escreve a regra de cada uma:

```json
"bases": {
  "Z": {
    "ler": true, "inserir": true, "alterar": true,
    "tabelas": {
      "folha":    { },
      "clientes": { "ler": true, "inserir": true, "alterar": true }
    }
  }
}
```

Nesse exemplo a Maria lê e grava tudo em `Z`, **menos** `folha`, onde não pode
nada.

### A regra da tabela SUBSTITUI a da base

Não soma, não corta: substitui — a mesma coisa que a base já fazia com o `"*"`.
É o que permite as **duas** coisas que a prática pede:

```json
// tirar uma tabela de quem lê o banco inteiro
"*": { "ler": true, "tabelas": { "folha": {} } }

// dar uma tabela a quem não lê o banco nenhum
"Z": { "tabelas": { "clientes": { "ler": true } } }
```

O segundo caso é o que uma regra de *interseção* não resolveria: se a tabela só
pudesse restringir, nunca daria para conceder uma tabela a quem não tem a base.

### A ordem, do mais específico para o mais geral

1. supervisor — pode tudo, em toda tabela;
2. a regra desta tabela nesta base;
3. a regra `"*"` de tabela nesta base;
4. a regra desta tabela na base `"*"`;
5. a regra `"*"` de tabela na base `"*"`;
6. e só então a regra da **base** — que por sua vez cai em `"*"` e no nível.

Operação que não fala de tabela — `bancos`, `criar_database`, `sistema` — cai
direto na regra da base, como sempre foi. **Um `config.json` sem `"tabelas"` se
comporta exatamente como antes**, e há teste que falha se deixar de se comportar.

### O que a tela mostra, e o que ela esconde

A árvore e o catálogo (`tabelas`, `sistabelas`, `siscolunas`) listam **só o que
dá para abrir**. Não é enfeite: sem isso, quem perdeu o direito a `folha`
continuaria vendo o nome dela na árvore, e o nome de uma tabela já conta parte
da história.

### As portas dos fundos que precisaram de conferência própria

O portão de permissão é **um só**, e ele lê o campo `"tabela"` do pedido. Duas
operações não têm esse campo:

- **`juntar`** — as tabelas estão em `a.tabela` e `b.tabela`;
- **`unir`** — as tabelas estão numa **lista** em `"tabelas"`.

Sem conferência própria, bastaria pedir a tabela negada como o lado B de uma
junção. As duas conferem cada tabela do pedido, e há teste para cada uma.'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''| `bases` | O poder, base por base. |''',
              '''| `bases` | O poder, base por base — e, dentro de cada base, tabela por tabela. |''', 1)

s = s.replace('''      "Z": {
        "ler": true, "inserir": true, "alterar": true, "excluir": false,
        "criar": false, "reindexar": false, "diario": true,
        "verificar": true, "administrar": false, "replicar": false
      }''','''      "Z": {
        "ler": true, "inserir": true, "alterar": true, "excluir": false,
        "criar": false, "reindexar": false, "diario": true,
        "verificar": true, "administrar": false, "replicar": false,
        "tabelas": { "folha": {} }
      }''', 1)

s = s.replace('''- **Sem grupos ou papéis.** O poder é por usuário. Com muitos usuários iguais,
  isso incomoda — e aí entram papéis.''','''- **Sem grupos ou papéis.** O poder é por usuário. Com muitos usuários iguais,
  isso incomoda — e aí entram papéis.
- **Sem direito por COLUNA.** O direito desce até a tabela, e para aí. Esconder
  uma coluna de salário dentro de uma tabela que a pessoa pode ler ainda não
  existe.''', 1)
p.write_text(s)
print("ok")
