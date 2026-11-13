<script lang="ts">
  import {
    store,
    addPerson,
    renamePerson,
    setPersonColor,
    deletePerson,
    toggleTrackPerson,
    isShared,
    instrumentLabel,
  } from './store.svelte'

  const people = $derived(store.song.people ?? [])
  const tracks = $derived(store.song.tracks)

  function plays(personId: string, trackId: string): boolean {
    const t = tracks.find((x) => x.id === trackId)
    return (t?.personIds ?? []).includes(personId)
  }

  function ownerCount(trackId: string): number {
    const t = tracks.find((x) => x.id === trackId)
    return t?.personIds?.length ?? 0
  }

  function onDelete(id: string, name: string) {
    if (!confirm(`Remover "${name}"? Os instrumentos e notas ficam; só o agrupamento sai.`)) return
    deletePerson(id)
  }
</script>

<div class="people-page">
  <div class="head">
    <div class="title-wrap">
      <h2>Pessoas</h2>
      <p class="sub">Uma pessoa toca um ou mais instrumentos. Marque os instrumentos de cada uma; um instrumento marcado por 2 pessoas fica <b>compartilhado</b>.</p>
    </div>
    <button class="add" onclick={() => addPerson()}>+ pessoa</button>
  </div>

  {#if people.length === 0}
    <p class="empty">Nenhuma pessoa ainda. Crie uma e marque os instrumentos dela. (´-ω-`)</p>
  {:else if tracks.length === 0}
    <p class="empty">Sem instrumentos. Adicione linhas no Editor primeiro.</p>
  {/if}

  <div class="cards">
    {#each people as person (person.id)}
      <section class="card" style:--pc={person.color ?? 'var(--accent)'}>
        <header class="card-head">
          <label class="swatch" title="Cor do grupo">
            <input
              type="color"
              value={person.color ?? '#e8663d'}
              oninput={(e) => setPersonColor(person.id, (e.currentTarget as HTMLInputElement).value)}
            />
          </label>
          <input
            class="pname"
            value={person.name}
            oninput={(e) => renamePerson(person.id, (e.currentTarget as HTMLInputElement).value)}
            spellcheck="false"
          />
          <label class="pen" title="Caneta: notas novas em instrumento compartilhado ganham esta pessoa">
            <input
              type="radio"
              name="active-person"
              checked={store.activePersonId === person.id}
              onchange={() => (store.activePersonId = person.id)}
            />
            caneta
          </label>
          <button class="del" title="Remover pessoa" onclick={() => onDelete(person.id, person.name)}>×</button>
        </header>

        <ul class="tracks">
          {#each tracks as t (t.id)}
            {@const on = plays(person.id, t.id)}
            <li class:on>
              <label>
                <input type="checkbox" checked={on} onchange={() => toggleTrackPerson(t.id, person.id)} />
                <span class="tn">{t.name}</span>
                <span class="ti">{instrumentLabel(t)}</span>
                {#if on && ownerCount(t.id) >= 2}<span class="shared" title="Compartilhado entre pessoas">compart.</span>{/if}
              </label>
            </li>
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  {#if tracks.some((t) => isShared(t))}
    <p class="legend">
      Instrumento <b>compartilhado</b>: use a <b>caneta</b> (acima) pra escrever notas de cada pessoa na grade,
      ou selecione notas e clique no chip da pessoa pra reatribuir. Cada nota soa e colore pela sua dona.
    </p>
  {/if}
</div>

<style>
  .people-page {
    padding: 0.5rem 0 1.5rem;
    max-width: 60rem;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  h2 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 800;
    color: var(--ink);
  }

  .sub {
    margin: 0.25rem 0 0;
    font-size: 0.78rem;
    color: var(--ink-soft);
    max-width: 40rem;
  }

  .add {
    flex: 0 0 auto;
    font: inherit;
    font-size: 0.8rem;
    font-weight: 700;
    padding: 0.4rem 0.9rem;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    cursor: pointer;
  }

  .add:hover {
    filter: brightness(1.05);
  }

  .empty {
    font-size: 0.85rem;
    color: var(--ink-soft);
  }

  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: 0.9rem;
  }

  .card {
    flex: 1 1 16rem;
    min-width: 15rem;
    max-width: 22rem;
    border: 1px solid var(--line);
    border-left: 4px solid var(--pc);
    border-radius: 10px;
    background: var(--paper);
    padding: 0.6rem 0.7rem 0.7rem;
  }

  .card-head {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.5rem;
  }

  .swatch {
    position: relative;
    width: 1.4rem;
    height: 1.4rem;
    border-radius: 5px;
    border: 1px solid var(--line-strong);
    background: var(--pc);
    overflow: hidden;
    cursor: pointer;
    flex: 0 0 auto;
  }

  .swatch input {
    position: absolute;
    inset: -4px;
    width: calc(100% + 8px);
    height: calc(100% + 8px);
    opacity: 0;
    cursor: pointer;
  }

  .pname {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-weight: 800;
    font-size: 0.9rem;
    color: var(--ink);
    background: transparent;
    border: none;
    border-bottom: 1px dashed transparent;
  }

  .pname:hover,
  .pname:focus-visible {
    border-bottom-color: var(--line-strong);
    outline: none;
  }

  .pen {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    font-size: 0.62rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--ink-soft);
    cursor: pointer;
  }

  .del {
    font: inherit;
    font-size: 1rem;
    font-weight: 700;
    border: none;
    background: transparent;
    color: var(--ink-faint);
    cursor: pointer;
    padding: 0 0.15rem;
  }

  .del:hover {
    color: var(--accent);
  }

  .tracks {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .tracks li label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0.3rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .tracks li:hover label {
    background: var(--paper-raise);
  }

  .tracks li.on label {
    background: color-mix(in srgb, var(--pc) 12%, transparent);
  }

  .tracks input[type='checkbox'] {
    accent-color: var(--pc);
  }

  .tn {
    font-weight: 700;
    color: var(--ink);
  }

  .ti {
    font-size: 0.68rem;
    color: var(--ink-soft);
  }

  .shared {
    margin-left: auto;
    font-size: 0.6rem;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--pc);
    border: 1px solid var(--pc);
    border-radius: 999px;
    padding: 0.05rem 0.4rem;
  }

  .legend {
    margin-top: 1rem;
    font-size: 0.75rem;
    color: var(--ink-soft);
    max-width: 44rem;
  }
</style>
