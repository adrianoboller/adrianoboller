// UI-001 — a tela de clientes, nova (o legado nao tinha tela). Fala com o
// clientes-rs pela API JSON; os campos e a ordem sao os do questionario F0:
// nome, e-mail, limite, ativo. Erros vem do backend com as MESMAS mensagens
// do PHP de 2011 -- a regra mora la, nao aqui.
import { useEffect, useState } from 'react'
import './App.css'

const API = import.meta.env.VITE_API ?? 'http://localhost:8080'

type Cliente = { id: number; nome: string; email: string; limite_credito: string; ativo: number }

export default function App() {
  const [lista, setLista] = useState<Cliente[]>([])
  const [todos, setTodos] = useState(false)
  const [form, setForm] = useState({ id: 0, nome: '', email: '', limite_credito: '' })
  const [erro, setErro] = useState('')

  const carregar = async () => {
    const r = await fetch(`${API}/clientes${todos ? '/todos' : ''}`)
    setLista(await r.json())
  }
  useEffect(() => { carregar() }, [todos])

  const salvar = async () => {
    setErro('')
    const novo = form.id === 0
    const r = await fetch(`${API}/clientes${novo ? '' : '/' + form.id}`, {
      method: novo ? 'POST' : 'PUT', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ nome: form.nome, email: form.email, limite_credito: form.limite_credito }),
    })
    const j = await r.json()
    if (j.erro) { setErro(j.erro); return }
    setForm({ id: 0, nome: '', email: '', limite_credito: '' })
    carregar()
  }
  const desativar = async (id: number) => {
    await fetch(`${API}/clientes/${id}`, { method: 'DELETE' }); carregar()
  }

  return (
    <main>
      <header><h1>Clientes</h1><span className="sub">Loja do Bairro · convertido do PHP de 2011</span></header>
      <section className="form">
        <label>Nome<input value={form.nome} onChange={e => setForm({ ...form, nome: e.target.value })} /></label>
        <label>E-mail<input value={form.email} onChange={e => setForm({ ...form, email: e.target.value })} /></label>
        <label>Limite de crédito<input inputMode="decimal" value={form.limite_credito} onChange={e => setForm({ ...form, limite_credito: e.target.value })} /></label>
        <div className="acoes">
          <button className={form.id ? 'alterar' : 'incluir'} onClick={salvar}>{form.id ? 'Alterar' : 'Incluir'}</button>
          {form.id !== 0 && <button className="voltar" onClick={() => setForm({ id: 0, nome: '', email: '', limite_credito: '' })}>Cancelar</button>}
        </div>
        {erro && <p className="erro" role="alert">{erro}</p>}
      </section>
      <section>
        <label className="filtro"><input type="checkbox" checked={todos} onChange={e => setTodos(e.target.checked)} /> mostrar desativados</label>
        <table>
          <thead><tr><th>#</th><th>Nome</th><th>E-mail</th><th className="num">Limite</th><th></th></tr></thead>
          <tbody>
            {lista.map(c => (
              <tr key={c.id} className={c.ativo ? '' : 'inativo'}>
                <td>{c.id}</td><td>{c.nome}</td><td>{c.email}</td><td className="num">{c.limite_credito}</td>
                <td><div className="acoes">
                  {c.ativo === 1 && <>
                    <button className="alterar" onClick={() => setForm({ id: c.id, nome: c.nome, email: c.email, limite_credito: c.limite_credito })}>Alterar</button>
                    <button className="marcar" onClick={() => desativar(c.id)}>Desativar</button>
                  </>}
                  {c.ativo === 0 && <span className="tag">desativado</span>}
                </div></td>
              </tr>
            ))}
            {lista.length === 0 && <tr><td colSpan={5} className="vazio">Nenhum cliente {todos ? 'cadastrado' : 'ativo'}.</td></tr>}
          </tbody>
        </table>
      </section>
    </main>
  )
}
