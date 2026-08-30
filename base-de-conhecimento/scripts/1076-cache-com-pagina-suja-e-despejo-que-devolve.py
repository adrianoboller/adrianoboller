# Cache com pagina suja e despejo que devolve
# 29/08 06:00

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()

# --- 1. a Entrada ganha o bit de suja ---
velho='''    fn por(&mut self, n: u64, bytes: &[u8]) {
        if let Some(e) = self.paginas.get_mut(&n) {
            e.bytes.clear();
            e.bytes.extend_from_slice(bytes);
            e.usada = true;
            return;
        }
        while self.paginas.len() >= self.teto {
            match self.fila.pop_front() {
                None => break,
                Some(velha) => match self.paginas.get_mut(&velha) {
                    None => {}
                    Some(e) if e.usada => {
                        e.usada = false;
                        self.fila.push_back(velha);
                    }
                    Some(_) => {
                        self.paginas.remove(&velha);
                    }
                },
            }
        }
        self.paginas.insert(
            n,
            Entrada {
                bytes: bytes.to_vec(),
                usada: false,
            },
        );
        self.fila.push_back(n);
    }'''
novo='''    /// Poe a pagina no cache. `suja` diz se ela ainda nao foi ao arquivo.
    ///
    /// Devolve a pagina SUJA que teve de sair para abrir lugar, se houve --
    /// quem chama e que sabe escrever no arquivo, e e ele que paga o CRC. Uma
    /// pagina limpa despejada nao devolve nada: o arquivo ja a tem.
    fn por(&mut self, n: u64, bytes: &[u8], suja: bool) -> Option<(u64, Vec<u8>)> {
        if let Some(e) = self.paginas.get_mut(&n) {
            e.bytes.clear();
            e.bytes.extend_from_slice(bytes);
            e.usada = true;
            e.suja |= suja;
            return None;
        }
        let mut despejada = None;
        while self.paginas.len() >= self.teto {
            match self.fila.pop_front() {
                None => break,
                Some(velha) => match self.paginas.get_mut(&velha) {
                    None => {}
                    Some(e) if e.usada => {
                        e.usada = false;
                        self.fila.push_back(velha);
                    }
                    Some(_) => {
                        let e = self.paginas.remove(&velha).unwrap();
                        if e.suja {
                            despejada = Some((velha, e.bytes));
                        }
                        break;
                    }
                },
            }
        }
        self.paginas.insert(
            n,
            Entrada {
                bytes: bytes.to_vec(),
                usada: false,
                suja,
            },
        );
        self.fila.push_back(n);
        despejada
    }

    /// Todas as sujas, ja marcadas como limpas. Quem chama grava e nao volta.
    fn tirar_sujas(&mut self) -> Vec<(u64, Vec<u8>)> {
        let mut fora = Vec::new();
        for (n, e) in self.paginas.iter_mut() {
            if e.suja {
                e.suja = false;
                fora.push((*n, e.bytes.clone()));
            }
        }
        // Em ordem de pagina: escrever para frente no arquivo em vez de saltar.
        fora.sort_unstable_by_key(|(n, _)| *n);
        fora
    }

    fn tem_suja(&self) -> bool {
        self.paginas.values().any(|e| e.suja)
    }'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('cache ok')
