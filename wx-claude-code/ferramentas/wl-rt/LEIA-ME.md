# wl-rt — o runtime mínimo do WLanguage para o perfil Rust

O código convertido do WLanguage precisa reproduzir a semântica **do legado**,
não a do `f64`, do `to_uppercase()` ou do `==` do Rust. Esta biblioteca é a
peça que faltava (item 7 da lista do que falta): pequena, `std` pura, e cada
função cita a página do Help WLanguage de onde a semântica foi **lida** — e cada
teste usa os exemplos dessa página como vetor.

```bash
cd ferramentas/wl-rt && cargo test      # 16 testes, todos com vetor do Help
```

| Módulo | O que cobre | Páginas do Help |
| --- | --- | --- |
| `moeda` | `currency`: ponto fixo de 6 casas em i128, `Round` metade para longe do zero, faixa de ±604 462 909 807 314 587,353087, `de_f64` com 15 dígitos significativos | 1514043, 3050063, 3050008 |
| `data` | AAAAMMDD: `DateValid`, `DateDifference`, `Data + n` soma dias, `DateSys` | 1514066, 3027003, 3027001, 3027026 |
| `texto` | `=` estrito, `~=` (caixa, acento, pontas), `~~` (espaços e pontuação), `NoSpace`, `Middle`, `Left`, `Right`, `Length`, `RepeatString`, `Replace`, `Upper`, `Truncate`, `Val` | 1512006, 3024034, 3024023, 3024001, 3024031, 3024036, 3024022, 3024039, 1000020476, 3024037 |
| `numero` | `Round` e `IntegerPart` sobre real, `NumToString` com máscara (`05d`, `+10.2f`, `010.2f`, `10.2fS`, `10,2fS`) | 3050063, 3050008, 3024024 |

## O que o Help ensinou (e o que o modelo teria errado)

- **`=` entre strings é estrito.** `"Dupond" = "DUPOND"` é falso. Quem ignora
  caixa, acento e espaços das pontas é `~=`; quem ignora também pontuação e
  todos os espaços é `~~`. Antes de ler a página, a suposição era o contrário.
- **`Upper("élan")` é `"ELAN"`**: o acento cai, não vira `É`.
- **`Left("AZELMA ")` sem tamanho tira os espaços finais.**
- **`Val("1D2")` é 100**: `D` é expoente, como `E`. `Val("7,5")` é 7.
- **A faixa do `currency` tem 18 dígitos inteiros**, não os «17» que o texto da
  página diz: o máximo publicado, 604 462 909 807 314 587,353087, é 2⁷⁹ − 1
  milionésimos. O limite conferido é o valor.
- **A página de `Replace` tem um erro de digitação**: imprime `"Abrococobro!"`
  para `Replace("Abracadabra!", "a", "o")`, e trocar só o `a` não transforma
  `d` em `c`. O teste guarda o resultado correto e a nota.

## Limites ditos

- `Right` sem tamanho não está no vetor lido; por isso o tamanho é obrigatório.
- `DateSys` sai em UTC; o WLanguage usa o fuso do sistema.
- `Upper` cobre o conjunto latino (é o que a página promete); `StringFormat`
  para o resto não está aqui.
- Não há `Numeric` de 38 dígitos, `Duration`, `DateTime` nem collation de
  ordenação — entram quando um projeto medir a necessidade.
