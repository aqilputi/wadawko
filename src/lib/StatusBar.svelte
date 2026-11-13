<script lang="ts">
  import { store } from './store.svelte'
  import { sectionRanges } from './store.svelte'

  const activeNames = $derived(
    store.selectedSections.length
      ? sectionRanges()
          .filter((s) => store.selectedSections.includes(s.id))
          .map((s) => s.name)
          .join(', ')
      : 'música inteira',
  )
</script>

<footer class="status">
  <span class="item">🥢 WaDAWko</span>
  <span class="sep">·</span>
  <span class="item">
    {#if store.playing && store.trainerOn && store.playbackBpm !== store.song.tempo}
      🏃 {store.playbackBpm} → {store.song.tempo} BPM
    {:else}
      {store.song.tempo} BPM
    {/if}
  </span>
  <span class="sep">·</span>
  <span class="item">tocando: {activeNames}</span>
  <span class="spacer"></span>
  <span class="item muted">salva automaticamente neste navegador</span>
  <span class="sep">·</span>
  <span class="item">
    samples:
    <a href="https://github.com/floe-audio/Taiko-Drums" target="_blank" rel="noreferrer">SCC Taiko Drums (CC BY-SA 4.0)</a>,
    <a href="https://freesound.org/people/whatsanickname4u/sounds/127337/" target="_blank" rel="noreferrer">whatsanickname4u (CC BY 3.0)</a>,
    <a href="https://freesound.org/people/RutgerMuller/sounds/364847/" target="_blank" rel="noreferrer">RutgerMuller (CC0)</a>
  </span>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.9rem;
    border-top: 1px solid var(--line);
    background: var(--paper);
    font-size: 0.7rem;
    color: var(--ink-soft);
  }

  .spacer {
    flex: 1;
  }

  .sep {
    color: var(--ink-faint);
  }

  .item.muted {
    color: var(--ink-faint);
  }

  .item a {
    color: var(--ink-soft);
    text-decoration: none;
  }

  .item a:hover {
    color: var(--accent);
  }
</style>
