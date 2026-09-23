# Bancada da união — o braço como PEDIDO (pedido 393)

Duas perguntas, uma corrida:

1. **Quanto custa filtrar FORA da união, quando dá para filtrar DENTRO?**
   A mesma resposta — `SELECT id,nome FROM g1 WHERE uf='SC' UNION SELECT
   id,nome FROM g2 WHERE uf='SC'` — por dois caminhos: `unir` por `tabelas`,
   que materializa as duas inteiras, e `unir` por `partes`, com um `buscar`
   por índice dentro de cada braço.
2. **Quanto um leitor inocente espera** enquanto uma união está em curso, por
   cada um dos dois caminhos.

## Como rodar

```bash
bancada/esta-medindo.sh && echo "ha medicao em curso -- espere"
cargo build --release --bin phxsqld     # binario velho mede o passado
python3 bancada/uniao/medir.py
```

O resultado vai para `resultados.json`, com **a data dentro do arquivo** — a
página dos testes lê dali, e nunca do `mtime`.

## As regras que esta bancada honra, e por que cada uma está aqui

- **Trabalho igual, não só pergunta igual.** A sonda confere que as duas
  respostas são **as mesmas linhas** antes de comparar tempo, e aborta a
  corrida se divergirem. Foi essa regra que reprovou o número anterior: o
  118,7× publicado em 22/09/2026 media, do lado filtrado, **dois `buscar`
  soltos** — o braço ainda não existia. O braço de verdade paga a máquina da
  união por cima da busca, e a razão caiu para **64,2×**. Número que compara
  simulação com operação é número que não vale.
- **Janela FIXA de pressão.** As três configurações do leitor sofrem a mesma
  duração de carga. Sem isso, «pior espera baixa» quer dizer só «a carga
  acabou antes» — e o caminho `partes` acaba ~54× mais rápido, então ele
  ganharia de graça.
- **Faixa min–max sempre, e vencedor só quando as faixas NÃO se cruzam.**
  O `resultados.json` grava `faixas_se_cruzam` e `leitor_faixas_se_cruzam`;
  quem lê julga em vez de acreditar.
- **Seletividade declarada.** 1% (`uf='SC'` a cada centésima linha), sem
  sorteio. O ganho é função dela: a 100% seria ~1×, e o número não quer dizer
  «o `unir` é 64× lento» — quer dizer «o `unir` obrigava a pagar a tabela
  inteira quando se quer 1% dela».

## O que ela NÃO mede

- **Vazão de união grande pelo caminho novo.** Um braço que traga a tabela
  inteira não tem índice para usar e cai no mesmo custo do caminho velho, sem
  a trava global — isso está nomeado e não medido.
- **Braço remoto (`dblink`).** Não existe.
