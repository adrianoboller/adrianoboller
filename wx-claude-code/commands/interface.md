---
description: "Qual a interface do Rust de destino (terminal, servico, desktop, web, mobile, IoT, TV, CarPlay) e o suporte medido no rustc local."
argument-hint: "[listar|manual [id]|escolher --opcao <id>]"
allowed-tools: "Read, Glob, Grep, Bash"
---

# A interface do programa que sai da conversão

O questionário diz a **linguagem** de destino. Não diz a **forma**: o mesmo
núcleo em Rust vira terminal, serviço de rede, janela, página, aplicativo de
celular ou firmware de placa — e cada uma exige uma ferramenta diferente e roda
num lugar diferente.

```bash
python3 "${CLAUDE_PLUGIN_ROOT}/skills/conversao-wx/scripts/interface_do_destino.py" \
  --project-root . ${1:-listar}
```

As nove formas: `terminal`, `servico-tcp`, `desktop`, `web`, `mobile`,
`iot-esp32`, `iot-arduino`, `smart-tv`, `carplay`.

O suporte **não vem de memória**. Ele sai de `rustc --print target-list` (o que
o compilador conhece) cruzado com `rustup target list` (o que tem `std`
pré-compilada). Alvo nos dois é tier 1/2 — `rustup target add` e pronto. Alvo só
no primeiro é **tier 3**: existe e compila, mas exige *nightly* e
`-Z build-std`, e não há CI da equipe do Rust por trás. Sem `rustc` na máquina,
o veredito é **INDISPONÍVEL** com o motivo, nunca um palpite.

Duas honestidades que o manual não esconde:

- **CarPlay não é alvo de compilação.** É a forma de apresentar um aplicativo
  iOS na tela do carro; o que se compila é `aarch64-apple-ios`.
- **Desktop com janela é a única forma que fura o zero-dependência**: o alvo é
  tier 1, mas desenhar a janela não vem da `std`.

`escolher` grava `.wx-migration/interface.json` e, quando existe
`questionario.json`, preenche `H_backend.interface` — **sem** criar pergunta
nova.
