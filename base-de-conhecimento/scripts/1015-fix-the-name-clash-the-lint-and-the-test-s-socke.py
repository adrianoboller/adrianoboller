# Fix the name clash, the lint and the test's socket close
# 29/08 02:58

import pathlib
p = pathlib.Path("bancada/carga/bulkinsert.py")
s = p.read_text()
s = s.replace('''    def matar(self):
        """Fecha o soquete na marra, sem soltar nada -- e o cliente que morreu."""
        self.s.close()''',
'''    def matar(self):
        """Mata a conexao na marra, sem soltar nada -- o cliente que morreu.

        SO_LINGER com timeout zero manda RST em vez de FIN: e o que acontece
        quando o processo do outro lado e morto, e nao quando ele se despede.

        E o `self.f.close()` nao e zelo: `makefile` segura o descritor, e
        fechar so o soquete deixa o fd ABERTO -- o servidor nunca veria o fim
        da conexao. Foi assim que a primeira versao deste teste passou por
        engano dizendo que a reserva nao soltava.
        """
        import struct
        self.s.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER,
                          struct.pack("ii", 1, 0))
        self.f.close()
        self.s.close()''',1)
p.write_text(s)
