# Add monitor field and dispatch
# 28/08 14:23

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()

# 1) campo no Servidor
a='''    remotos: Mutex<HashMap<String, Arc<Mutex<Remoto>>>>,
    conexoes: AtomicUsize,
}'''
b='''    remotos: Mutex<HashMap<String, Arc<Mutex<Remoto>>>>,
    /// Amostra anterior da maquina, para as taxas do painel.
    ///
    /// Guardar aqui, e nao na tela, e o que permite dizer "CPU em 40%": o
    /// `/proc` so traz contadores desde o arranque, e taxa exige duas
    /// amostras. Uma unica trava para todos os navegadores tambem evita cada
    /// aba abrir a propria serie e nenhuma delas fechar conta.
    monitor: Mutex<crate::sistema::Monitor>,
    /// Ultimo aviso mandado por caminho, para nao repetir enquanto o disco
    /// continua cheio.
    avisados: Mutex<HashMap<String, i64>>,
    conexoes: AtomicUsize,
}'''
assert a in s; s=s.replace(a,b,1)

a='''            remotos: Mutex::new(HashMap::new()),
            conexoes: AtomicUsize::new(0),'''
b='''            remotos: Mutex::new(HashMap::new()),
            monitor: Mutex::new(crate::sistema::Monitor::novo()),
            avisados: Mutex::new(HashMap::new()),
            conexoes: AtomicUsize::new(0),'''
assert a in s; s=s.replace(a,b,1)

# 2) despacho
a='''            "painel" => self.op_painel(sessao),'''
b='''            "painel" => self.op_painel(sessao),
            "sistema" => Ok(self.op_sistema()),'''
assert a in s; s=s.replace(a,b,1)

# 3) relogio
a='''        self.subir_backup_agendado();
        self.ligar_relogio_de_gravacao();'''
b='''        self.subir_backup_agendado();
        self.ligar_relogio_de_gravacao();
        self.ligar_vigia_de_disco();'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')
