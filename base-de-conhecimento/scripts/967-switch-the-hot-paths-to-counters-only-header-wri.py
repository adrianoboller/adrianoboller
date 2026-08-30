# Switch the hot paths to counters-only header writes
# 29/08 01:08

import pathlib
p = pathlib.Path("crates/phxsql-store/src/reg.rs")
s = p.read_text()

s = s.replace('''    fn gravar_contadores(&mut self) -> Result<()> {
        let buf = self.montar_cabecalho(1);
        self.volumes.escrever(1, 0, &buf)
    }''','''    fn gravar_contadores(&mut self, volume: u32) -> Result<()> {
        let buf = self.montar_cabecalho(volume);
        self.volumes.escrever(volume, 0, &buf)
    }''',1)

linhas = s.split("\n")
# indices 0-based das linhas a trocar (as de 1-based acima)
for n in [429, 438, 469, 839, 841, 896, 1010, 1026]:
    i = n - 1
    if "gravar_cabecalho(1)" in linhas[i]:
        linhas[i] = linhas[i].replace("gravar_cabecalho(1)", "gravar_contadores(1)")
    elif "gravar_cabecalho(volume)" in linhas[i]:
        linhas[i] = linhas[i].replace("gravar_cabecalho(volume)", "gravar_contadores(volume)")
    else:
        raise SystemExit(f"linha {n} nao era o que eu esperava: {linhas[i]!r}")
s = "\n".join(linhas)
p.write_text(s)
print("ok")
