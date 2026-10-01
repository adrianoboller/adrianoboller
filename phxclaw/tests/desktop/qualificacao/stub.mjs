export function stubTauriFn(grade) {
  const ouvintes = {};
  window.__chamadas = [];
  const emitir = (ev, payload) => (ouvintes[ev] || []).forEach(f => f({ payload }));
  window.__emitir = emitir;
  window.__TAURI__ = {
    event: { listen: async (ev, f) => { (ouvintes[ev] ||= []).push(f); return () => {}; } },
    core: { invoke: async (cmd, args) => {
      window.__chamadas.push({ cmd, args });
      switch (cmd) {
        case 'host_status': return { version: '0.0.0-stub', session_uuid: '01900000-0000-7000-8000-000000000000', policy: {}, live_bus: { receiver_count: 1 }, api: { addr: '127.0.0.1:8787' } };
        case 'verify_evidence': return { valid: true, records: 1 };
        case 'events_snapshot': return [{ topic: 'task', event_type: 'created', uuid: '01900000-0000-7000-8000-0000000000aa', occurred_at: '2026-10-01T09:00:00Z' }];
        case 'terminal_abrir': { const id = `t${window.__chamadas.length}`; emitir('terminal_grade', { id, ...grade }); return { id, pid: 4242, programa: args.programa, cwd: '/projeto' }; }
        default: return null;
      }
    } },
  };
}

