# Add the lease config
# 29/08 02:53

import pathlib
p = pathlib.Path("crates/phxsql-server/src/config.rs")
s = p.read_text()
s = s.replace('''    /// Usuarios DIFERENTES conectados ao mesmo tempo. Zero = sem teto.''',
'''    /// Minutos que uma reserva de carga (`BULKINSERT`) dura sem ser renovada.
    ///
    /// E a SEGUNDA rede de protecao contra reserva orfa. A primeira e a queda
    /// da conexao, que solta na hora; esta pega o caso em que o soquete fica
    /// pendurado vivo com o cliente morto do outro lado.
    ///
    /// Zero nao desliga: cairia no padrao, porque reserva sem prazo nenhum e
    /// exatamente a que trava a tabela para sempre.
    pub carga_prazo_min: u64,
    /// Usuarios DIFERENTES conectados ao mesmo tempo. Zero = sem teto.''',1)
s = s.replace('''            conexoes_max: 64,
            usuarios_max: 0,
        }''','''            conexoes_max: 64,
            carga_prazo_min: 30,
            usuarios_max: 0,
        }''',1)
s = s.replace('''            cache_paginas: r''','''            carga_prazo_min: {
                let m = r.inteiro_ou("carga_prazo_min", padrao.carga_prazo_min as i64);
                if m > 0 { m as u64 } else { padrao.carga_prazo_min }
            },
            cache_paginas: r''',1)
s = s.replace('''            ("cache_paginas", Json::de_u64(self.cache_paginas as u64)),''',
'''            ("cache_paginas", Json::de_u64(self.cache_paginas as u64)),
            ("carga_prazo_min", Json::de_u64(self.carga_prazo_min)),''',1)
p.write_text(s)
print("config ok")
