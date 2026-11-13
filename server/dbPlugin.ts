import { mkdirSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import type { IncomingMessage, ServerResponse } from 'node:http'
import type { Plugin, Connect } from 'vite'

/**
 * Banco SQLite local (data/wadawko.db) exposto como um key-value simples em /api/kv.
 * Roda dentro do próprio servidor do Vite (dev e preview), então os projetos não
 * dependem mais do endereço/origem usado no navegador.
 *
 *   GET    /api/kv        → { [key]: value }
 *   PUT    /api/kv/:key   → grava o corpo (texto) como valor
 *   DELETE /api/kv/:key   → remove
 */
export function dbPlugin(file = 'data/wadawko.db'): Plugin {
  let db: DatabaseSync | null = null
  const open = () => {
    if (db) return db
    const path = resolve(file)
    mkdirSync(dirname(path), { recursive: true })
    db = new DatabaseSync(path)
    db.exec(`
      PRAGMA journal_mode = WAL;
      CREATE TABLE IF NOT EXISTS kv (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at INTEGER NOT NULL
      );
    `)
    return db
  }

  const readBody = (req: IncomingMessage) =>
    new Promise<string>((ok, fail) => {
      const chunks: Buffer[] = []
      req.on('data', (c: Buffer) => chunks.push(c))
      req.on('end', () => ok(Buffer.concat(chunks).toString('utf8')))
      req.on('error', fail)
    })

  const send = (res: ServerResponse, status: number, body?: unknown) => {
    res.statusCode = status
    if (body === undefined) return res.end()
    res.setHeader('Content-Type', 'application/json')
    res.end(JSON.stringify(body))
  }

  const handler: Connect.NextHandleFunction = async (req, res, next) => {
    const url = new URL(req.url ?? '/', 'http://x')
    if (!url.pathname.startsWith('/api/kv')) return next()
    try {
      const d = open()
      const key = decodeURIComponent(url.pathname.slice('/api/kv/'.length))
      if (req.method === 'GET' && !key) {
        const rows = d.prepare('SELECT key, value FROM kv').all() as { key: string; value: string }[]
        return send(res, 200, Object.fromEntries(rows.map((r) => [r.key, r.value])))
      }
      if (!key) return send(res, 400, { error: 'key obrigatória' })
      if (req.method === 'PUT' || req.method === 'POST') {
        const value = await readBody(req)
        d.prepare(
          `INSERT INTO kv (key, value, updated_at) VALUES (?, ?, ?)
           ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at`,
        ).run(key, value, Date.now())
        return send(res, 204)
      }
      if (req.method === 'DELETE') {
        d.prepare('DELETE FROM kv WHERE key = ?').run(key)
        return send(res, 204)
      }
      return send(res, 405, { error: 'método não suportado' })
    } catch (e) {
      console.error('[db]', e)
      return send(res, 500, { error: String(e) })
    }
  }

  return {
    name: 'wadawko-db',
    configureServer(server) {
      server.middlewares.use(handler)
    },
    configurePreviewServer(server) {
      server.middlewares.use(handler)
    },
  }
}
