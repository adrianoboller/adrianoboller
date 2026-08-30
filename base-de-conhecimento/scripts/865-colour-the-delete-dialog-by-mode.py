# Colour the delete dialog by mode
# 28/08 22:49

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()
antigo = """    const sim = fundo.querySelector("#btExcSim");
    sim.textContent = modo === "suave" ? "Marcar como excluído" : "Excluir de vez";
    sim.classList.toggle("perigo", modo === "fisico");"""
novo = """    const sim = fundo.querySelector("#btExcSim");
    sim.textContent = modo === "suave" ? "Marcar como excluído" : "Excluir de vez";
    // Rosa para a que volta, vermelho para a que não volta. O botão troca de
    // cor junto com o texto: quem clica sem ler o texto ainda vê a cor mudar.
    sim.classList.toggle("marcar", modo === "suave");
    sim.classList.toggle("excluir", modo === "fisico");"""
assert antigo in s
s = s.replace(antigo, novo)
s = s.replace('<button class="botao" id="btExcSim">Marcar como excluído</button>',
              '<button class="botao marcar" id="btExcSim">Marcar como excluído</button>')

# a chip de visao "excluidas" tambem ganha o rosa, e a legenda entra na grade
antigo = """      <span class="chip-visao">
        <button id="vwAtivas" class="${verExcluidos ? "" : "ativo"}">ativas</button>
        <button id="vwExcl" class="${verExcluidos ? "ativo" : ""}">excluídas</button>
      </span>"""
novo = """      <span class="chip-visao">
        <button id="vwAtivas" class="${verExcluidos ? "" : "ativo"}">ativas</button>
        <button id="vwExcl" class="marcadas ${verExcluidos ? "ativo" : ""}">excluídas</button>
      </span>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")
