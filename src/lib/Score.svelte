<script lang="ts">
  import { store } from './store.svelte'
  import { renderScore, type HighlightMap } from './score'
  import { engine } from './audio'
  import { cellFloatAt } from './lane'
  import { cellsPerMeasure } from './types'

  let root: HTMLElement
  let highlights: HighlightMap = new Map()

  /** duração do brilho por nota, proporcional ao andamento (~1 colcheia) */
  const flashMs = $derived(Math.min(400, Math.max(140, 30000 / store.song.tempo)))

  function flash(key: string) {
    const els = highlights.get(key)
    if (!els) return
    for (const el of els) {
      el.classList.add('hl')
      setTimeout(() => el.classList.remove('hl'), flashMs)
    }
  }

  $effect(() => {
    JSON.stringify(store.song)
    try {
      highlights = renderScore(root, $state.snapshot(store.song))
    } catch (err) {
      console.error('renderScore falhou:', err)
    }
  })

  // enquanto a partitura está visível, cada golpe do playback acende sua nota
  $effect(() => {
    engine.setHitListener(flash)
    return () => engine.setHitListener(null)
  })

  // auto-scroll suave seguindo o playback (aprox. linear por fração de célula)
  $effect(() => {
    if (!store.playing) return
    let raf = 0
    const step = () => {
      const song = store.song
      const total = song.measures.reduce((s, m) => s + cellsPerMeasure(m), 0)
      if (total > 0 && root) {
        const frac = cellFloatAt(song, engine.transportSeconds()) / total
        const max = root.scrollWidth - root.clientWidth
        root.scrollLeft = Math.max(0, Math.min(max, frac * root.scrollWidth - root.clientWidth * 0.4))
      }
      raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
    return () => cancelAnimationFrame(raf)
  })
</script>

<p class="legend">
  acima da linha = mão direita · abaixo = mão esquerda · <b>×</b> = borda (fuchi) · <b>&gt;</b> = acento · <i style="color:#d6452b">itálico vermelho</i> = kiai (grito)
</p>
<div class="score-view" bind:this={root}></div>

<style>
  .legend {
    font-size: 0.72rem;
    color: var(--ink-soft);
    margin: 0 0 0.5rem;
  }

  .score-view {
    overflow-x: auto;
    padding: 0.5rem 0 1rem;
  }

  .score-view :global(.score-row) {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
  }

  .score-view :global(.score-labels) {
    flex: 0 0 11.5rem;
    border-right: 3px solid var(--ink);
    padding-top: 10px;
    align-self: stretch;
  }

  /* gutter de trilhos de pessoa (alinhado às pautas: TOP=10px, ROW_H=110px) */
  .score-view :global(.score-rails) {
    display: flex;
    gap: 3px;
    padding-top: 10px;
    margin-right: 0.5rem;
    align-self: flex-start;
  }

  .score-view :global(.score-rail) {
    position: relative;
    width: 0.5rem;
    display: flex;
    flex-direction: column;
  }

  .score-view :global(.rail-seg) {
    height: 110px;
    border-radius: 3px;
    background: transparent;
  }

  .score-view :global(.rail-seg.on) {
    background: var(--pc);
  }

  .score-view :global(.rail-name) {
    position: absolute;
    top: 2px;
    left: 50%;
    transform: translateX(-50%) rotate(180deg);
    writing-mode: vertical-rl;
    font-size: 0.6rem;
    font-weight: 800;
    color: #fff;
    background: var(--pc);
    border-radius: 0 0 3px 3px;
    padding: 0.25rem 0.01rem;
    max-height: 6rem;
    overflow: hidden;
    white-space: nowrap;
    z-index: 2;
  }

  .score-view :global(.score-label) {
    height: 110px;
    display: flex;
    align-items: center;
    padding: 0 0.5rem;
    font-weight: 700;
    font-size: 0.85rem;
  }

  .score-view :global(.score-staves svg) {
    display: block;
  }

  /* nota acesa pelo golpe do playback */
  .score-view :global(.hl),
  .score-view :global(.hl *) {
    fill: var(--accent);
  }

  .score-view :global(.hl [stroke]:not([stroke='none'])),
  .score-view :global(.hl[stroke]:not([stroke='none'])) {
    stroke: var(--accent);
  }
</style>
