<script lang="ts">
  import { store, railsForTracks, instrumentLabel } from './store.svelte'
  import { engine } from './audio'
  import { noteEvents } from './lane'

  const rails = $derived(railsForTracks())

  /** velocidade constante da esteira (px/s). BPM alto = notas mais juntas. */
  const PX_PER_SEC = 380

  let root: HTMLElement
  let fieldWidth = 600

  const laneData = $derived(noteEvents(store.song))
  const eventsByTrack = $derived(
    store.song.tracks.map((t) => ({
      track: t,
      notes: laneData.events.filter((e) => e.trackId === t.id),
    })),
  )

  // nós de nota e tambores (recoletados quando a música muda; $effect roda após o DOM)
  let noteNodes: { el: HTMLElement; sec: number; base: number }[] = []
  let drums = new Map<string, HTMLElement>()

  $effect(() => {
    laneData // dependência: re-coleta quando as notas mudam
    if (!root) return
    noteNodes = [...root.querySelectorAll<HTMLElement>('.note')].map((el) => ({
      el,
      sec: Number(el.dataset.sec),
      // ghost = disco apagado; dono mudo = ainda mais apagado (nota silenciada)
      base: (el.classList.contains('ghost') ? 0.5 : 1) * (el.dataset.muted === '1' ? 0.3 : 1),
    }))
    drums = new Map()
    for (const el of root.querySelectorAll<HTMLElement>('.drum')) {
      if (el.dataset.track) drums.set(el.dataset.track, el)
    }
  })

  // largura do campo de rolagem (define quanto tempo a nota fica visível)
  $effect(() => {
    if (!root) return
    const measure = () => {
      const f = root.querySelector<HTMLElement>('.field')
      if (f) fieldWidth = f.clientWidth
    }
    measure()
    const ro = new ResizeObserver(measure)
    ro.observe(root)
    return () => ro.disconnect()
  })

  // loop de animação: lê o relógio do transport e posiciona cada disco
  $effect(() => {
    let raf = 0
    const frame = () => {
      const now = engine.transportSeconds()
      const w = fieldWidth
      for (const n of noteNodes) {
        const x = (n.sec - now) * PX_PER_SEC
        if (x > w + 48 || x < -90) {
          n.el.style.visibility = 'hidden'
          continue
        }
        n.el.style.visibility = 'visible'
        n.el.style.transform = `translate3d(${x}px, -50%, 0)`
        // some ao passar da linha de julgamento (à esquerda, consumida);
        // ghost já entra apagado (base < 1)
        const fade = x < 0 ? Math.max(0, 1 + x / 80) : 1
        n.el.style.opacity = String(fade * n.base)
      }
      raf = requestAnimationFrame(frame)
    }
    raf = requestAnimationFrame(frame)
    return () => cancelAnimationFrame(raf)
  })

  // cada golpe do playback acende o tambor da sua linha (sincronizado ao áudio)
  function pulse(key: string) {
    const trackId = key.slice(0, key.indexOf(':'))
    const drum = drums.get(trackId)
    if (!drum) return
    drum.classList.remove('hit')
    void drum.offsetWidth // força reflow: golpes seguidos re-disparam a animação
    drum.classList.add('hit')
    setTimeout(() => drum.classList.remove('hit'), 240)
    spawnPetals(drum)
  }

  // explosão de pétalas de cerejeira a cada golpe 🌸
  const PETAL_COLORS = ['#ffd7e8', '#ffb7d5', '#ff9ec4', '#ffc9de', '#fce4ec']
  function spawnPetals(drum: HTMLElement) {
    const count = 8 + Math.floor(Math.random() * 5) // 8–12 pétalas
    for (let i = 0; i < count; i++) {
      const petal = document.createElement('span')
      petal.className = 'petal'
      const size = 6 + Math.random() * 7
      petal.style.width = `${size}px`
      petal.style.height = `${size * 0.82}px`
      petal.style.background = PETAL_COLORS[Math.floor(Math.random() * PETAL_COLORS.length)]
      drum.appendChild(petal)

      // trajetória: estoura pra fora do tambor e cai flutuando pra baixo
      const angle = -Math.PI / 2 + (Math.random() - 0.5) * Math.PI * 1.6
      const burst = 26 + Math.random() * 44
      const bx = Math.cos(angle) * burst
      const by = Math.sin(angle) * burst
      const fall = 34 + Math.random() * 30
      const drift = (Math.random() - 0.5) * 36
      const spin = (Math.random() - 0.5) * 720
      const dur = 700 + Math.random() * 500

      const anim = petal.animate(
        [
          { transform: 'translate(0, 0) rotate(0deg) scale(1)', opacity: 1 },
          {
            transform: `translate(${bx}px, ${by}px) rotate(${spin * 0.4}deg) scale(1)`,
            opacity: 1,
            offset: 0.35,
          },
          {
            transform: `translate(${bx + drift}px, ${by + fall}px) rotate(${spin}deg) scale(0.7)`,
            opacity: 0,
          },
        ],
        { duration: dur, easing: 'cubic-bezier(0.2, 0.7, 0.4, 1)' },
      )
      anim.onfinish = () => petal.remove()
    }
  }

  $effect(() => {
    engine.setHitListener(pulse)
    return () => engine.setHitListener(null)
  })
</script>

<div class="lane-view" bind:this={root}>
  {#each eventsByTrack as { track, notes }, ti (track.id)}
    <div class="lane">
      {#if rails.length}
        <div class="rail-gutter">
          {#each rails as rail (rail.person.id)}
            {@const owns = rail.rows[ti]}
            {@const capTop = owns && !(ti > 0 && rail.rows[ti - 1])}
            <div class="rail" title={rail.person.name}>
              {#if owns}<span class="bar" class:extend={rail.rows[ti + 1]} class:cap-top={capTop} style:--pc={rail.person.color ?? 'var(--accent)'}></span>{/if}
              {#if capTop}<span class="rail-name" style:--pc={rail.person.color ?? 'var(--accent)'}>{rail.person.name}</span>{/if}
            </div>
          {/each}
        </div>
      {/if}
      <div class="drum" data-track={track.id} title={instrumentLabel(track)}>
        <span class="drum-face"></span>
        <span class="drum-name">{track.name}</span>
      </div>
      <div class="field">
        <div class="judge"></div>
        {#each notes as note (note.key)}
          <div
            class="note {note.zone === 'rim' ? 'fuchi' : 'couro'}"
            class:accent={note.accent}
            class:ghost={note.ghost}
            class:both={note.bothHands}
            class:owned={!!note.ownerColor}
            style:--owner={note.ownerColor ?? 'transparent'}
            data-sec={note.sec}
            data-muted={note.ownerMuted ? '1' : '0'}
            title={note.label}
          >
            {note.label}
          </div>
        {/each}
      </div>
    </div>
  {/each}

  {#if laneData.events.length === 0}
    <p class="empty">Sem notas ainda. Escreva na grade, depois volte pra tocar. (´-ω-`)</p>
  {/if}
</div>

<style>
  .lane-view {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.5rem 0 1rem;
  }

  .lane {
    display: flex;
    align-items: stretch;
    gap: 0;
    height: 4.4rem;
  }

  /* gutter de agrupamento por pessoa: barra vertical por lane, filha da lane
     (alinha sempre); `extend` cobre o gap de 0.6rem → linha contínua */
  .rail-gutter {
    flex: 0 0 auto;
    display: flex;
    gap: 0.3rem;
    padding-right: 0.4rem;
    align-items: stretch;
  }

  .rail {
    position: relative;
    width: 0.8rem;
  }

  .bar {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--pc);
    border-radius: 0;
  }

  /* arredonda só as pontas do trecho; junções internas quadradas = linha contínua */
  .bar.cap-top {
    border-top-left-radius: 2px;
    border-top-right-radius: 2px;
  }

  .bar:not(.extend) {
    border-bottom-left-radius: 2px;
    border-bottom-right-radius: 2px;
  }

  /* liga ao próximo segmento cobrindo o gap de 0.6rem entre lanes */
  .bar.extend {
    bottom: -0.62rem;
  }

  .rail-name {
    position: absolute;
    top: 0.1rem;
    left: 4px;
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    font-size: 0.6rem;
    font-weight: 800;
    color: var(--pc);
    white-space: nowrap;
    pointer-events: none;
    z-index: 2;
  }

  /* tambor = alvo de julgamento, à esquerda */
  .drum {
    flex: 0 0 4.4rem;
    position: relative;
    display: grid;
    place-items: center;
    z-index: 2;
  }

  .drum-face {
    width: 3.2rem;
    height: 3.2rem;
    border-radius: 50%;
    border: 3px solid var(--ink);
    background: radial-gradient(circle at 38% 34%, var(--paper-raise), var(--paper-hover));
    box-shadow: inset 0 0 0 4px var(--paper), 0 2px 6px rgba(0, 0, 0, 0.18);
    transition:
      transform 60ms ease-out,
      box-shadow 60ms ease-out;
  }

  .drum-name {
    position: absolute;
    bottom: -0.15rem;
    font-size: 0.58rem;
    font-weight: 700;
    color: var(--ink-soft);
    white-space: nowrap;
  }

  /* pulso no golpe */
  .drum:global(.hit) .drum-face {
    transform: scale(1.16);
    box-shadow:
      inset 0 0 0 4px var(--paper),
      0 0 0 3px var(--accent),
      0 2px 10px rgba(0, 0, 0, 0.25);
  }

  .drum:global(.hit)::after {
    content: '';
    position: absolute;
    width: 3.2rem;
    height: 3.2rem;
    border-radius: 50%;
    border: 3px solid var(--accent);
    animation: burst 240ms ease-out forwards;
    pointer-events: none;
  }

  /* pétala de cerejeira: elipse com "bico" (formato de pétala) */
  .drum :global(.petal) {
    position: absolute;
    top: 50%;
    left: 50%;
    border-radius: 80% 10% 80% 10%;
    pointer-events: none;
    z-index: 3;
    box-shadow: 0 0 2px rgba(255, 158, 196, 0.5);
  }

  @keyframes burst {
    from {
      transform: scale(1);
      opacity: 0.85;
    }
    to {
      transform: scale(2.1);
      opacity: 0;
    }
  }

  .field {
    position: relative;
    flex: 1 1 auto;
    overflow: hidden;
    border: 1px solid var(--line);
    border-left: none;
    border-radius: 0 6px 6px 0;
    background: repeating-linear-gradient(
      90deg,
      var(--paper-raise),
      var(--paper-raise) 1px,
      var(--paper) 1px,
      var(--paper) 3.2rem
    );
  }

  /* linha de julgamento (onde a nota deve ser tocada) */
  .judge {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--ink);
    opacity: 0.35;
    z-index: 1;
  }

  .note {
    position: absolute;
    left: 0;
    top: 50%;
    width: 2.2rem;
    height: 2.2rem;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 0.62rem;
    font-weight: 700;
    color: #fff;
    border: 2px solid rgba(255, 255, 255, 0.85);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
    will-change: transform;
    visibility: hidden;
    user-select: none;
  }

  /* couro (head) = vermelho · fuchi (rim) = azul */
  .note.couro {
    background: var(--accent);
  }

  .note.fuchi {
    background: #2f6fb0;
  }

  .note.accent {
    width: 2.9rem;
    height: 2.9rem;
    font-size: 0.68rem;
    border-width: 3px;
  }

  /* duas mãos no mesmo instante (doron): disco grande com anel duplo */
  .note.both {
    width: 3.1rem;
    height: 3.1rem;
    font-size: 0.66rem;
    border-width: 3px;
    box-shadow:
      inset 0 0 0 3px rgba(255, 255, 255, 0.9),
      0 0 0 2px var(--ink),
      0 1px 5px rgba(0, 0, 0, 0.3);
  }

  /* instrumento compartilhado: anel na cor do dono ao redor do disco */
  .note.owned {
    box-shadow:
      0 0 0 3px var(--owner),
      0 1px 4px rgba(0, 0, 0, 0.3);
  }

  /* ghost note (tsu/ku): disco pequeno e apagado */
  .note.ghost {
    width: 1.2rem;
    height: 1.2rem;
    font-size: 0;
    border-width: 1px;
    opacity: 0.5;
    box-shadow: none;
  }

  .empty {
    font-size: 0.8rem;
    color: var(--ink-soft);
    padding: 1rem 0.5rem;
  }
</style>
