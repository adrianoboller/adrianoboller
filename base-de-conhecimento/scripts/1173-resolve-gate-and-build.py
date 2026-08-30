# Resolve gate and build
# 29/08 18:08

import re, pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
m = re.search(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", t, re.S)
head = m.group(1)
# O 2b do HEAD ja existe logo abaixo (cluster + somente_leitura); do RAMO
# aproveita-se apenas a leitura VIVA, que agora e o mesmo campo.
t = t.replace(m.group(0), head, 1)
# E o 2b passa a ler o valor vivo pelo metodo, em vez do campo direto: o
# `somente_leitura()` e o mesmo atomico, e a tela de configuracao tambem o
# escreve agora.
t = t.replace("""            } else if self.somente_leitura_vivo.load(Ordering::Relaxed) {
                return Err(PhxError::Autorizacao(self.msg("erro.somente_leitura", &[])));""",
"""            } else if self.somente_leitura() {
                // Le o valor VIVO, que dois caminhos escrevem: a promocao de um
                // spare e a gravacao pela tela de configuracao.
                return Err(PhxError::Autorizacao(self.msg("erro.somente_leitura", &[])));""", 1)
p.write_text(t); print("portao resolvido; marcas:", t.count("<<<<<<<"))
