/**
 * Persistência dos projetos no banco SQLite do servidor (/api/kv, ver server/dbPlugin.ts).
 *
 * A API é síncrona (get/set/remove) para o store continuar simples: tudo é carregado
 * em memória no boot e as gravações vão para o servidor em segundo plano (com debounce).
 * O localStorage fica como cópia local: se o servidor estiver fora, o app usa ele e
 * marca as chaves como pendentes, que são enviadas na próxima vez que o banco responder.
 */

const API = '/api/kv'
const PREFIX = 'wadawko:'
const DIRTY_KEY = 'wadawko:__dirty'

const cache = new Map<string, string | null>()
const pending = new Map<string, ReturnType<typeof setTimeout>>()
let chain: Promise<unknown> = Promise.resolve()

export const dbStatus = { online: false }

function lsGet(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}
function lsSet(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key)
    else localStorage.setItem(key, value)
  } catch {
    /* sem espaço / bloqueado: o banco continua sendo a fonte */
  }
}

function dirtyKeys(): Set<string> {
  try {
    return new Set(JSON.parse(lsGet(DIRTY_KEY) ?? '[]') as string[])
  } catch {
    return new Set()
  }
}
function markDirty(key: string, dirty: boolean) {
  const s = dirtyKeys()
  if (dirty) s.add(key)
  else s.delete(key)
  lsSet(DIRTY_KEY, s.size ? JSON.stringify([...s]) : null)
}

function localWadawkoKeys(): string[] {
  const keys: string[] = []
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i)
      if (k?.startsWith(PREFIX) && k !== DIRTY_KEY && k !== 'wadawko:theme') keys.push(k)
    }
  } catch {
    /* ignora */
  }
  return keys
}

async function send(key: string, keepalive = false) {
  const value = cache.get(key) ?? null
  const url = `${API}/${encodeURIComponent(key)}`
  const res = await fetch(
    url,
    value === null ? { method: 'DELETE', keepalive } : { method: 'PUT', body: value, keepalive },
  )
  if (!res.ok) throw new Error(`${res.status}`)
}

function push(key: string) {
  chain = chain.then(async () => {
    try {
      await send(key)
      markDirty(key, false)
      dbStatus.online = true
    } catch (e) {
      dbStatus.online = false
      console.warn('[db] falha ao gravar, tentando de novo em 5s', key, e)
      schedule(key, 5000)
    }
  })
}

function schedule(key: string, delay = 400) {
  const t = pending.get(key)
  if (t) clearTimeout(t)
  pending.set(
    key,
    setTimeout(() => {
      pending.delete(key)
      push(key)
    }, delay),
  )
}

/** carrega tudo do banco; sem servidor, cai pro localStorage */
export async function loadDb() {
  try {
    const res = await fetch(API)
    if (!res.ok) throw new Error(`${res.status}`)
    const data = (await res.json()) as Record<string, string>
    for (const [k, v] of Object.entries(data)) cache.set(k, v)
    dbStatus.online = true

    // mudanças feitas offline (ou banco vazio no 1º uso): o local vence
    const local = Object.keys(data).some((k) => k.startsWith(PREFIX)) ? [...dirtyKeys()] : localWadawkoKeys()
    for (const k of local) {
      cache.set(k, lsGet(k))
      push(k)
    }
    for (const [k, v] of cache) lsSet(k, v)
  } catch (e) {
    console.warn('[db] servidor indisponível, usando localStorage', e)
    dbStatus.online = false
    for (const k of localWadawkoKeys()) cache.set(k, lsGet(k))
  }
}

export function dbGet(key: string): string | null {
  return cache.get(key) ?? null
}

export function dbSet(key: string, value: string) {
  cache.set(key, value)
  lsSet(key, value)
  markDirty(key, true)
  schedule(key)
}

export function dbRemove(key: string) {
  cache.set(key, null)
  lsSet(key, null)
  markDirty(key, true)
  schedule(key)
}

// fechando a aba: manda o que ainda está no debounce
addEventListener('pagehide', () => {
  for (const [key, t] of pending) {
    clearTimeout(t)
    send(key, true).then(() => markDirty(key, false)).catch(() => {})
  }
  pending.clear()
})
