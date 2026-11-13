<script lang="ts">
  import { store, exportSong, newProject, switchProject, duplicateProject, deleteProject } from './store.svelte'

  import { exportMp3 } from './exportAudio'

  let { onImport }: { onImport: () => void } = $props()

  let mp3Status: string | null = $state(null)

  async function onExportMp3() {
    if (mp3Status) return
    try {
      await exportMp3($state.snapshot(store.song), (s) => (mp3Status = s))
    } catch (err) {
      console.error('export mp3 falhou:', err)
      alert('Não deu pra exportar o MP3.')
    } finally {
      mp3Status = null
    }
  }

  const projects = $derived([...store.workspace.projects].sort((a, b) => b.updatedAt - a.updatedAt))

  function fmt(ts: number): string {
    try {
      return new Date(ts).toLocaleDateString(undefined, { day: '2-digit', month: 'short' })
    } catch {
      return ''
    }
  }

  function onDelete(id: string, title: string) {
    if (store.workspace.projects.length <= 1) {
      if (!confirm('Excluir a única música? Uma nova em branco será criada.')) return
    } else if (!confirm(`Excluir "${title}"? Não dá pra desfazer.`)) return
    deleteProject(id)
  }
</script>

<aside class="right" aria-label="Músicas">
  <div class="head">
    <span class="title">Músicas</span>
    <button class="new" title="Nova música" onclick={newProject}>+ nova</button>
  </div>

  <ul class="list">
    {#each projects as p (p.id)}
      <li>
        <div
          class="song"
          class:current={p.id === store.workspace.currentId}
          role="button"
          tabindex="0"
          onclick={() => switchProject(p.id)}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && switchProject(p.id)}
        >
          <div class="info">
            <span class="name">{p.title || 'Sem título'}</span>
            <span class="meta">{fmt(p.updatedAt)}</span>
          </div>
          <div class="row-actions">
            <button class="mini" title="Duplicar" onclick={(e) => { e.stopPropagation(); duplicateProject(p.id) }}>⧉</button>
            <button class="mini danger" title="Excluir" onclick={(e) => { e.stopPropagation(); onDelete(p.id, p.title) }}>×</button>
          </div>
        </div>
      </li>
    {/each}
  </ul>

  <div class="foot">
    <button onclick={exportSong}>exportar</button>
    <button onclick={onImport}>importar</button>
  </div>
  <div class="foot">
    <button onclick={onExportMp3} disabled={!!mp3Status} title="Renderiza a música (com repetições e dinâmica) em MP3">
      {mp3Status ?? 'exportar mp3'}
    </button>
  </div>
</aside>

<style>
  .right {
    width: 260px;
    flex: 0 0 260px;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.75rem;
    background: var(--paper);
    border-left: 1px solid var(--line);
    overflow-y: auto;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .title {
    font-weight: 800;
    font-size: 0.85rem;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    color: var(--ink-soft);
  }

  .new {
    font: inherit;
    font-size: 0.75rem;
    font-weight: 700;
    padding: 0.25rem 0.7rem;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .new:hover {
    filter: brightness(1.05);
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    flex: 1;
  }

  .song {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.4rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--paper-raise);
    cursor: pointer;
  }

  .song:hover {
    border-color: var(--line-strong);
  }

  .song.current {
    border-color: var(--accent);
    background: var(--accent-wash);
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .name {
    font-weight: 700;
    font-size: 0.85rem;
    color: var(--ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta {
    font-size: 0.68rem;
    color: var(--ink-soft);
  }

  .row-actions {
    display: flex;
    gap: 0.15rem;
    opacity: 0;
    transition: opacity 100ms;
  }

  .song:hover .row-actions,
  .song.current .row-actions {
    opacity: 1;
  }

  .mini {
    font: inherit;
    font-size: 0.75rem;
    width: 1.5rem;
    height: 1.5rem;
    border: 1px solid var(--line);
    background: var(--paper);
    color: var(--ink-soft);
    border-radius: 4px;
    cursor: pointer;
  }

  .mini:hover {
    color: var(--ink);
  }

  .mini.danger:hover {
    color: var(--accent);
    border-color: var(--accent);
  }

  .foot {
    display: flex;
    gap: 0.3rem;
  }

  .foot button {
    flex: 1;
    font: inherit;
    font-size: 0.75rem;
    padding: 0.35rem 0.6rem;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    background: var(--paper-raise);
    color: var(--ink);
    cursor: pointer;
  }

  .foot button:disabled {
    opacity: 0.7;
    cursor: progress;
  }

  .foot button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
