---
type: Plan
title: "App nativo em Rust + GPUI"
description: "Plano do porte nativo: inventário do app web, arquitetura de crates, áudio em tempo real, desempenho de UI, notação, fases e riscos."
tags: [planejamento, nativo, gpui, arquitetura]
status: stable
estado: "fases 0–5 implementadas; fase 6 (bonecos) adiada; fase 7 em aberto"
---

# WaDAWko nativo — Planejamento Rust + GPUI

> Reescrita do **WaDAWko** (hoje Svelte 5 + Vite + TypeScript, Tone.js, VexFlow, @breezystack/lamejs e um `server/` de persistência) como app desktop nativo em Rust, com **GPUI** (framework de UI do Zed) e áudio em **thread de tempo real** via cpal. Inclui o escopo novo de **bonecos animados** (personagens 2D por Pessoa, estilo Sukeroku) sincronizados à música.

Data: 2026-11-10 · Status: planejamento, pré-código · Plataforma-alvo: Linux (CachyOS), cross-platform desejável · Base: `/planejamento/inicial.md`, `/planejamento/redesign-web.md`, `/planejamento/instrumentos.md`, `/planejamento/pessoas.md` + leitura de `src/`, `server/`, `data/`, `scripts/`, `public/`.

> **Atualização 2026-11-10:** bonecos animados (§6, Fase 6) **adiados** a pedido — fora do escopo por enquanto; a seção fica como referência. Spike da Fase 0 implementado em `spike/` (ver §8.0).

---

## 0. Decisões em uma página

| Tema | Decisão |
|---|---|
| UI | GPUI via os snapshots publicados **`gpui-pre` 0.3.12 + `gpui-component` 0.9.1** (ambos de 2026-11-09), não o `gpui 0.2.2` do crates.io (out/2025, defasado). Grade, esteira, partitura e palco dos bonecos são **Elements customizados** com culling de viewport, nunca um elemento por célula/nota. |
| Áudio | **Engine próprio** sobre `cpal`; kira descartado; `fundsp` no máximo como biblioteca de DSP. Callback sem alocação/lock; comandos por ring buffer SPSC (`rtrb`) + atômicos. Render offline = o mesmo engine rodando sem dispositivo (paridade live/export). |
| Dados | JSON `version: 1` idêntico ao atual (serde espelhando `src/lib/types.ts`), persistido em SQLite (`rusqlite`) na mesma tabela `kv` de `data/wadawko.db`. O banco atual abre sem migração. |
| Notação | Engraving próprio (subconjunto) com a fonte SMuFL **Bravura**, desenhado por um trait `Canvas2D` com backends GPUI (tela), SVG (impressão/export) e tiny-skia (PNG/vídeo). |
| Bonecos | Rig 2D vetorial (cut-out) com keyframes + easing, no mesmo `Canvas2D`; golpes automáticos derivados das notas com antecipação; lane de movimentos por pessoa; relógio = posição em samples do thread de áudio; vídeo via frames tiny-skia + ffmpeg. |
| `server/` | Eliminado. O app abre o SQLite direto. |
| Fase 0 | Spike descartável com critérios de go/no-go. Plano B: **iced + canvas** (reaproveita todos os crates não-UI). |

---

## 1. Inventário do app atual

### 1.1 O que existe e funciona hoje (por arquivo)

**Domínio (`src/lib/types.ts`, 233 linhas)** — `Song{version:1,title,tempo,swing?,tracks,measures,sections?,people?,instruments?,mainTrackIds?}`; `Measure{beats,subdivision(2|4|8),cells[trackId][],cellOwners?,dynamic?(pp..ff),hairpin?(cresc|dim),trackDynamics?,kyai?}`; `Track{id,name,instrument,instrumentId?,muted,solo,volume?,pan?,personIds?,kyai?{pitch,vary,gain}}`; `Person{id,name,color?,muted?,solo?,volume?}`; `InstrumentDef{id,name,base,builtin?,head?,rim?}` com `ZoneParams{pitch,gain,tone{type,cutoff,q},overtones,decay}`; `Section{id,name,start,repeat?}`; 8 instrumentos base (`shime,chu,hira,nagado,onagado,okedo,odaiko,kane`) + 5 presets builtin; `DYNAMIC_LEVELS` (pp 0.25 … ff 3.2); `PERSON_COLORS`.

**Vocabulário e frases (`vocabulary.ts`, `phrase.ts`)** — 20 sílabas com golpes `{offset 0|0.5, zone head|rim, vel, hand R|L}` (DON/don/KON/kon/doko/doron/do/ko/tsu/ku/tsuku/KA/ka/ra/kara/karan/ten/teke/su); `PALETTE_GROUPS`; 7 `PHRASES` do repertório (cavalinho, dondoko, jiuchi dontsuku, tontsuton, matsuri, jiuchi shime, rulo). `splitWord` quebra palavras coladas com backtracking, absorve alongamento (`dooon` → stretch 3) e caixa alta = acento; `parseWords`; marca de sforzando `!`; `resolveCell` (memoizado) distribui golpes dentro da célula por peso de stretch.

**Timeline (`timeline.ts`)** — `playOrder` (desdobra ritornellos `section.repeat`, respeita região de compassos), `mainTracks/mainMul` (destaque ×f/mf e ×p/mf), `dynamicsCurve` (herança de marcação, rampas de hairpin até a próxima marcação ou um degrau, override por instrumento).

**Store (`store.svelte.ts`, 945 linhas)** — workspace multi-música (`wadawko:workspace` + `wadawko:project:<id>`), trocar/nova/duplicar/excluir; seções (append, clone, mover com os compassos, renomear, repeat 1..16, remover); dinâmicas por compasso/instrumento; +/− compasso dentro da seção selecionada; `setSubdivision` reamostrando células e kiai; `setCell/fillSequential/fillPhraseBeats/fillKyai`; linhas (add por InstrumentDef, remover, renomear, trocar instrumento, mover, toggle main); pessoas (CRUD, `toggleTrackPerson`, `assignCellOwner`, `setCellsOwner`, `railsForTracks`, `personAudible`); kit (CRUD de defs custom, `resolveInstrument` com fallback); export/import `.taiko.json`; demo "Renshū".

**Engine de áudio (`audio.ts`, 1185 linhas, Tone.js)** — `SAMPLE_FILES` (74 `.ogg` em camadas de velocity × round robin por instrumento/zona), `TUNING`, `TAIL` (cauda máxima e tempo de choke), `EQ` por instrumento/zona (highpass/lowshelf/peaking), `velGain` (curva de velocity com piso p/ ghost notes), humanização (ganho ±12 %, pitch ±1 %, timing ±6 ms), seleção de camada por `vel·√dyn`, round robin, **choke** (couro abafa couro+borda; borda abafa borda), filtro de timbre/overtones/decay/pitch/ganho por zona da `InstrumentDef`, `Tone.Channel` por linha (vol/pan/mute) e por dono em linha compartilhada, **barramento por pessoa**, mudo/solo em 2 camadas, agendamento no `Transport` em `bars:beats:sixteenths` com `swingWarp`, cursor por célula, `hitListener` (`trackId:célulaGlobal:golpe`), kiai agendado, metrônomo (synth), loop com região, seek ao vivo, count-in (4 cliques), **trainer** (BPM sobe a cada volta), previews (célula, zona do instrumento, loop de frase no hover, kiai), **`renderOffline`** (janelas de 2 s, mudo/solo, limiter com lookahead `lookaheadLimit`), master (−6 dB → compressor → soft clip), easter egg `playFue` (Sakura Sakura), `transportSeconds` dobrado pro compasso-fonte.

**Kiai (`kyai.ts`, 421 linhas)** — `splitKyai` (silabação japonesa romaji), `kyaiProfile` por pessoa (hash do id → pitch/velocidade/grit/sopro/extensão), `contour` (entonação, `!` = pico que cai), `playKyai` (recorta letras do `animalese.ogg`, transpõe por playbackRate, ruído filtrado nas sibilantes), 11 `KYAI_PRESETS`.

**Export (`exportAudio.ts`)** — MP3 192 kbps via lamejs com normalização de pico, cedendo o thread entre blocos.

**UI** — `App.svelte` (790): shell, topbar (título, BPM, reto/swing, loop, metrônomo, count-in, trainer com %/passo, play, tema), páginas editor/mixer/pessoas/kit, abas grade/partitura/esteira, paleta flutuante ancorada à célula focada (hover de frase toca preview após 2 s), command palette `Ctrl+K` (18 comandos), undo/redo (100 snapshots JSON), autosave 400 ms, tema claro/escuro persistido. `Grid.svelte` (**2434 linhas**, o monólito): blocos de seção com cabeçalho (nome, ritornello, colapsar em aba, arrastar pra reordenar, clique seleciona seções pra tocar), linha de timeline (arrastar = pan, clique = playhead/`startCell`, seek ao vivo), rótulos de contagem, células como `<input>` (Tab/espaço avança, setas, Enter, Esc, botão direito apaga, flash ao escrever), segmentos desenhados pra sílabas multi-golpe, zoom (`store.zoom` escala `--cellw`), auto-follow no playback, linha de dinâmica por compasso (select pp–ff + hairpin; linhas por instrumento), sub-linha de kiai por instrumento (datalist, popover de voz pitch/vary/gain, testar), popover de edição da linha, **modo seleção** (retângulo, mover/clonar blocos, copiar/recortar/colar, Delete, `!` alterna sfz, atribuir seleção a pessoa), gutter de trilhos de pessoa, hint de dono, transparência de dono mudo. `Score.svelte`+`score.ts` (VexFlow): pauta de 1 linha por tambor, D acima/E abaixo da linha, cabeça × = borda, acento, `sfz`, sílabas (doko → do+ko), kiai em itálico vermelho, quiálteras, beams, pausas, `StaveConnector`, cor do dono, highlight por golpe, auto-scroll. `Lane.svelte`+`lane.ts`: esteira estilo Taiko no Tatsujin (discos couro/fuchi, acento, ghost, anel de duas mãos, anel do dono), 380 px/s, tambor pulsa no golpe, **pétalas de cerejeira** por golpe. `MixerPage` (strips por pessoa + barramento), `PeoplePanel` (cards, cor, checklist de instrumentos), `KitPage` (builtins + custom, sliders por zona, preview), `RightPanel` (lista de músicas, exportar/importar JSON, exportar MP3), `NavRail`, `StatusBar`, `CommandPalette`, `ContextToolbar` (modo, subdivisão, +seção, +linha, caneta de pessoa).

**Infra** — `server/dbPlugin.ts`: middleware do Vite expondo `data/wadawko.db` (SQLite, tabela `kv(key,value,updated_at)`) em `/api/kv`; `src/lib/db.ts`: cache em memória + espelho em localStorage + fila de pendências. `scripts/extract-samples.py`: pipeline offline (ffmpeg + numpy) que gerou os samples a partir do SCC Taiko Drums, taikoset1 e gongo de Kashiwa — **não faz parte do app**, continua como ferramenta. `public/samples/`: 74 `.ogg` + `animalese.ogg` + `CREDITS.md` (CC BY-SA 4.0 / CC BY 3.0 / CC0 / CC BY 4.0). `data/`: `wadawko.db` (+WAL de 4 MB não checkpointado) e `backup-localstorage-2026-10-02.json` (4 projetos + workspace + tema). Fontes via Google Fonts (Shippori Mincho, Zen Kaku Gothic New).

### 1.2 Só planejado (não implementado)

Share por URL (deflate+base64), PWA/offline, CSS de impressão, export MIDI/TJA/SVG/PNG/PDF, MIDI input, painéis destacáveis, presets de layout, i18n, histórico visual, arranger, seções por referência (matriz Renoise), Inspector contextual (o painel direito só lista músicas), página Config (slot inativo no NavRail), **vocabulário editável por grupo** (`VOCABULARY` é constante), dinâmica/hairpin por célula (hoje só por compasso), mão D/E colorida na grade, gravar os taikos do grupo, e **anotação de movimento/kata** (backlog da Fase 5 do `/planejamento/inicial.md`) — que vira a feature de bonecos da seção 6.

### 1.3 Mapa feature → equivalente Rust

| Feature | Hoje | Rust | Notas |
|---|---|---|---|
| Modelo `Song` v1 | `types.ts` | `wadawko-core::model` (serde, camelCase, `skip_serializing_if`, `#[serde(flatten)] extra`) | Mesmo JSON; campos desconhecidos fazem round-trip |
| Vocabulário/frases | `vocabulary.ts` | `core::vocab` (tabela estática + futura edição por música) | Preparar `Song.vocabulary?` opcional |
| `splitWord/resolveCell/sfz` | `phrase.ts` | `core::phrase` (memo via `HashMap`) | Testes de tabela com as 20 sílabas e casos `dooon`, `dokon`, `DOKO` |
| `playOrder/dynamicsCurve/mainMul` | `timeline.ts` | `core::timeline` | Porte 1:1, testes com fixtures do backup |
| Operações de música | `store.svelte.ts` | `core::ops::{sections,measures,cells,tracks,people,kit}` funções puras sobre `&mut Song` | Testáveis sem UI |
| Workspace/projetos, autosave | `db.ts` + `server/dbPlugin.ts` | `wadawko-store` (rusqlite, tabela `kv`) | Abre `data/wadawko.db` como está |
| Undo/redo | snapshots JSON em `App.svelte` | `store::History<Arc<Song>>` (100 níveis) | Clone de `Song` é barato (KBs) |
| Transport, agendamento, swing, loop, região, seek, count-in, trainer | `Engine` (`audio.ts`) | `wadawko-audio::{transport,schedule}` no thread RT | Posições em ticks (PPQ 960); BPM muda ao vivo |
| Sampler (layers/RR/choke/EQ/humanização/decay/tone) | `makeVoice/playLayered/choke` | `audio::{voice,bank,dsp}` pool fixo de vozes + biquads | Sem alocação no callback |
| Canais por linha/dono, barramento por pessoa, mudo/solo 2 camadas | `ensureVoice/applyMix` | `audio::mix` (tabela de canais fixa, comandos `SetMix`) | |
| Metrônomo | `Tone.Synth` | voz sintética (triângulo + envelope) | |
| Kiai | `kyai.ts` | `core::kyai` (silabação, perfil, contorno) + `audio::kyai_voice` (granular sobre `animalese.ogg`) | O recorte/pitch é precomputado no UI |
| Master (compressor + soft clip) e limiter de export | `addMasterLimiter/lookaheadLimit` | `audio::dsp::{compressor,softclip,lookahead_limiter}` | Porte direto das fórmulas |
| Render offline | `renderOffline` | `audio::offline::render(song) -> Vec<f32>` | Mesmo engine, sem janelas/suspend |
| MP3 | lamejs | `mp3lame-encoder` 0.3 (LGPL) em thread com progresso | Alternativa: ffmpeg CLI |
| WAV | — | `hound` | Novo (16/24-bit) |
| Decodificação de samples | `decodeAudioData` | `symphonia` (ogg+vorbis) na carga; assets embutidos com `include_bytes!`/`rust-embed` | ~4 MB no binário |
| Grade (edição) | `Grid.svelte` | `app::views::grid::GridElement` (Element customizado) + um único input flutuante | Seção 4 |
| Paleta flutuante, command palette, toolbar, painel direito, status | `.svelte` | `gpui-component` (Popover, Input, Select, List, Dock) + views próprias | |
| Esteira | `Lane.svelte` | `app::views::lane::LaneElement` (canvas, partículas) | |
| Partitura | VexFlow | `wadawko-notation` + Bravura | Seção 5 |
| Mixer / Pessoas / Kit | `.svelte` | views com Slider/ColorPicker/Checkbox do gpui-component | |
| Tema claro/escuro | CSS vars | `app::theme` (tokens de `app.css`) + `gpui-component` Theme | Mesmas cores |
| Fontes | Google Fonts | Zen Kaku Gothic New + Shippori Mincho (OFL) embutidas | |
| Easter egg fue 🌸 | `playFue` | voz sintética extra | Barato; manter |
| Bonecos | — | `wadawko-anim` + `app::views::stage` | Seção 6 |

---

## 2. Estado do GPUI — verificado vs. assumido

**Verificado hoje (2026-11-10) na API do crates.io e docs.rs:**

- `gpui` oficial no crates.io: **0.2.2 (2025-10-22)**, Apache-2.0, sem MSRV declarada; só 3 releases não-yanked em toda a história. docs.rs mostra docs rasas (crate-level curto, maioria dos itens com uma linha, aponta para gpui.rs/Discord/fonte). Existe `canvas()` ("acesso à API de pintura de baixo nível"), `PaintQuad`/`paint_quad`, `Path`/`PathBuilder`, `uniform_list` e `list` (virtualização), `ScrollWheelEvent`, `Animation`. A doc oficial do GPUI cita um crate `gpui_platform` com features `wayland`/`x11` — **ele não existe no crates.io (404)**: a documentação está à frente do que é publicado. Sinal claro de churn.
- `gpui-pre` **0.3.12 (2026-11-09)**: "snapshot of zed@d2a4fb6", publicado toda semana pela Longbridge (0.3.9 em 12/10, 0.3.10 em 26/10, 0.3.11 em 02/11). 458 k downloads.
- `gpui-fast` **0.1.14 (2026-11-09)**: fork da Longbridge "com o layout reworked para manter o que não mudou entre frames" — relevante para a nossa preocupação com o playhead.
- `gpui-component` **0.9.1** e `gpui-kit` **0.9.1 (2026-11-09)**, repositório renomeado para `longbridge/gpui-kit` (20 k estrelas, Apache-2.0). `gpui-component` depende de **`gpui-pre =0.3.12`** (ou `gpui-fast` opcional). Anuncia 60+ componentes: listas virtuais (altura variável), tabelas com scroll virtual, dock com painéis redimensionáveis e abas, inputs, dropdowns, temas, AccessKit, charts, Markdown. Plataformas: macOS, Windows, Linux (e wasm experimental).
- Comunidade fragmentada: existem forks `gpui-ce`, `gpui-box`, `fc-gpui`, `xgpui` no crates.io, com issues abertas sobre não conseguir desabilitar X11 ou Wayland separadamente. Não usar nenhum deles.
- Áudio: `cpal` 0.18.2 (2026-08), `kira` 0.12.6 (2026-10), `mp3lame-encoder` 0.3.0 (2026-11, LGPL-3.0), `fundsp` 0.23, `symphonia` 0.6.x. Não re-verificados hoje (conhecidos): `hound` 3.5, `rtrb` 0.3, `rusqlite`, `tiny-skia` 0.11, `rubato`.

**Assumido (confirmar no spike):**

- Nomes exatos da API em `gpui-pre` 0.3.12: `canvas(prepaint, paint)`, `Window::paint_quad/paint_path/paint_image`, `PathBuilder::fill/stroke`, `window.request_animation_frame()`, `div().on_scroll_wheel`, `window.text_system().shape_line` + `ShapedLine::paint`, `with_element_state` para cache por elemento, `TransformationMatrix` em imagens/SVG.
- Comportamento no Linux: Zed roda em Wayland e X11 desde 2024 com o renderizador **blade (Vulkan)** e `cosmic-text`; scaling fracionário no Wayland tem histórico de bugs. Precisa de driver Vulkan funcional (Mesa) — no CachyOS isso é padrão.
- Custo real de `render()` por frame (GPUI reexecuta a árvore a cada frame desenhado); a hipótese do plano é que, com caches em `ElementState`, o custo por frame fica O(células visíveis).
- Entrada de texto: há `Input` no gpui-component; não precisamos de IME (kuchi shōga é romaji).

---

## 3. Arquitetura

### 3.1 Workspace de crates

```
wadawko/
  Cargo.toml                 # workspace, resolver 2, lints comuns
  crates/
    wadawko-core/            # domínio puro: model (serde), vocab, phrase, timeline, ops, kyai (texto), choreo (modelo). Zero deps de UI/áudio
    wadawko-samples/         # assets embutidos (ogg + manifest) e decodificação (symphonia) → SampleBank {Arc<SampleData>}
    wadawko-audio/           # engine RT: cpal, transport, schedule compiler, voice pool, dsp, mix, kiai_voice, metronome, offline render
    wadawko-export/          # WAV (hound), MP3 (mp3lame-encoder), vídeo (ffmpeg pipe)
    wadawko-store/           # rusqlite kv, workspace/projetos, autosave debounced, import de backup, History
    wadawko-draw/            # trait Canvas2D {quad, path, text, image, clip, transform} + backends: gpui, tiny-skia, svg
    wadawko-notation/        # engraving subset (SMuFL/Bravura) → Canvas2D
    wadawko-anim/            # rig 2D, keyframes/easing, strike generator, avaliação de coreografia → Canvas2D
    wadawko-app/             # binário GPUI: shell, theme, views (grid, lane, score, stage, mixer, people, kit, choreo), commands, keymap
    wadawko-cli/             # utilitário: list/export/render/import-backup (testes sem UI)
  assets/
    fonts/   ZenKakuGothicNew-*.ttf, ShipporiMincho-*.ttf, Bravura.otf (+ bravura_metadata.json)
    samples/ (cópia/symlink de public/samples, com CREDITS.md)
    moves/   *.ron (biblioteca de movimentos dos bonecos)
  tools/     scripts/extract-samples.py (continua Python; não muda)
```

Dependência de UI: `gpui = { package = "gpui-pre", version = "=0.3.12" }` e `gpui-component = "=0.9.1"` (mesmo pin que o gpui-component usa, senão não compila junto). Trocar o pin só em dia marcado, com o changelog aberto. Alternativa equivalente: `gpui` por git `rev` fixado no monorepo do Zed, sem gpui-component — só vale se o kit nos atrapalhar.

### 3.2 Thread da GUI × thread de áudio em tempo real

```
GUI thread (GPUI)                                   Audio thread (callback do cpal)
──────────────────────                               ──────────────────────────────
Entity<Project> { song, ui }                         Engine {
  │ edita Song                                          transport: {playing, pos_ticks: f64, bpm, loop, count_in}
  │ compila ──▶ Arc<Schedule>  ──(rtrb SPSC)──▶        schedule: Arc<Schedule>  (eventos ordenados por tick)
  │ Command::{Play, Stop, Seek, SetBpm, SetLoop,        voices: [Voice; 128]      (pool fixo, roubo da mais antiga)
  │           SetMix(TrackMix), SetPersonMix,           channels: [Channel; 64], buses: [Bus; 16], master
  │           Preview(Hit), SetMetronome, Trainer}      biquads por (track, zona): até 5 por zona, estado fixo
  │                                                      choke rings: [[Ringing; 16]; 64·2]
  ◀──(rtrb)── Event::{HitFired{key,sample}, LoopWrapped, Ended, Xrun}
  ◀── atomics: sample_pos (AtomicU64), stamp (Instant em ns, AtomicU64), bpm (AtomicU32 bits), playing
  ◀──(rtrb "lixo")── Arc<Schedule> antigo, pra dropar fora do callback
```

Regras do callback (invioláveis): sem `alloc`, sem `Mutex`, sem log, sem syscalls, sem `Arc::drop` da última referência (os `Arc<Schedule>` e `Arc<SampleData>` antigos voltam pela fila de lixo e são dropados na GUI). Tudo preallocado na criação do engine. `cpal` com `SampleFormat::F32`, buffer pedido de 256–512 frames a 48 kHz (configurável), callback de erro conta xruns.

**Schedule compilado na GUI** (porte de `Engine.schedule` + `renderOffline` de `audio.ts`): `Vec<HitEvent{tick, voice_kind, sample_ref(idx), layer_vel, dyn, gain, rate, zone, channel, bus, key}>` já com `playOrder` desdobrado, `dynamicsCurve` avaliada, `mainMul`, `sfzDyn`, `swingWarp` aplicado, round robin **pré-escolhido** (determinístico por seed) e os eventos de kiai (`KyaiNote` precomputado com letras/durações/contorno) e de metrônomo. Posições em **ticks** (PPQ 960) para o BPM poder mudar ao vivo (trainer, input de BPM) como no `Transport` do Tone. Edição durante o playback: a GUI recompila (debounce 50 ms) e envia um `Arc<Schedule>` novo; o thread de áudio reposiciona `next_idx` por busca binária no tick atual — preserva a sensação de step sequencer do `$effect` atual.

**Humanização** (`jitter` de ganho/pitch/timing) vira um PRNG xorshift no thread de áudio (sem `rand` com alocação). Choke e `TAIL` idênticos (fade por rampa de ganho na voz, não `stop`).

### 3.3 Engine/sequencer: kira × fundsp × engine próprio

| Opção | Prós | Contras para o WaDAWko |
|---|---|---|
| **kira** 0.12 | Clocks, tweens, mixer tracks, efeitos (filtro, compressor, EQ), backend cpal pronto, ativo | Semântica de "jogo": sem round robin/velocity layers/choke (seriam nossos de qualquer forma), sem render offline para arquivo (backend mock é p/ teste), agendamento por clock não dá o controle sample-exato de ritornello/região/seek/trainer que `Engine.schedule` já tem; brigaríamos com a API |
| **fundsp** 0.23 | DSP de grafo elegante, filtros/compressores prontos, `sequence` | É DSP, não sequencer; construir grafos aloca (ok fora do callback), mas vozes dinâmicas por golpe não se encaixam; útil só como caixa de ferramentas |
| **Engine próprio sobre cpal** (recomendado) | O `audio.ts` **já é** um engine próprio sobre primitivas Web Audio; o porte é 1:1; controle total de alocação; o mesmo `process()` roda live e offline (paridade garantida — hoje `OfflineAudioContext` diverge) | Escrever biquads RBJ (~80 linhas), compressor, soft clip, interpolação cúbica de sample, pool de vozes (~1,5 k linhas no total) |

**Recomendação: engine próprio.** `fundsp` fica como dependência opcional para o easter egg do fue e para validar nossos filtros em testes. Nada de kira.

### 3.4 Samples

- Manter os 74 `.ogg` (+ `animalese.ogg`) e o `CREDITS.md`; decodificar com `symphonia` (features `ogg`, `vorbis`) na inicialização, em thread, para `SampleData{frames: Vec<f32> estéreo intercalado, sr: 48000}`. ~30 MB em RAM, decodificação < 1 s.
- Embutir no binário (`rust-embed`) para o app ser um arquivo só; flag `--samples-dir` para desenvolvimento/samples do grupo.
- Taxa do dispositivo ≠ 48 kHz: a voz já toca por `rate` com interpolação cúbica; multiplicar por `48000/sr_device`. Sem `rubato`.
- `SAMPLE_FILES`, `TUNING`, `TAIL`, `EQ` de `audio.ts` viram um `manifest.ron` (o `scripts/extract-samples.py` já gerava um `manifest.json` equivalente).

### 3.5 Export

- **Render offline**: `wadawko-audio::offline::render(&Song, sr) -> Stereo<Vec<f32>>` cria um `Engine` sem dispositivo, carrega o mesmo `Schedule`, chama `process()` em blocos de 512 até `fim + 3 s` de cauda, aplica `lookahead_limit` (porte de `lookaheadLimit`, teto −1,5 dBFS, pré-ganho até +12 dB). Mudo/solo respeitados na compilação. Roda em thread com progresso e cancelamento.
- **WAV**: `hound`, 16-bit por padrão, 24-bit opcional.
- **MP3**: `mp3lame-encoder` 0.3 (192 kbps joint stereo, mesma normalização de pico do `exportAudio.ts`). É LGPL-3.0 (bindings + libmp3lame): aceitável para o projeto; alternativa sem LGPL embutida: `ffmpeg -i out.wav -b:a 192k out.mp3` (o projeto já depende de ffmpeg nas ferramentas).
- Mais tarde (fora deste plano): MIDI (`midly`), TJA.

### 3.6 Modelo de dados e persistência

- `Song` em serde com `rename_all = "camelCase"`, `Option` + `skip_serializing_if` em tudo que é opcional no TS, `#[serde(flatten)] extra: BTreeMap<String, serde_json::Value>` em `Song`, `Track`, `Measure`, `Person` para round-trip de chaves desconhecidas. `version: 1` validado na leitura (como `readSong`). Campos novos dos bonecos (seção 6) entram **opcionais**, mantendo `version: 1`, exatamente como `people`/`instruments` foram adicionados.
- **SQLite**: `rusqlite` (feature `bundled`), mesma tabela `kv(key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)` e as mesmas chaves `wadawko:workspace`, `wadawko:project:<id>`, `wadawko:theme`. Caminho padrão `$XDG_DATA_HOME/wadawko/wadawko.db`; `--db ./data/wadawko.db` para usar o banco do repositório (abre como está, `PRAGMA journal_mode=WAL`, checkpoint ao fechar — o `-wal` de 4 MB atual some). Comando `wadawko-cli import-backup data/backup-localstorage-2026-10-02.json` (o arquivo é um dicionário chave → string JSON, igual ao `GET /api/kv`).
- Autosave com debounce de 400 ms, escrita em thread de I/O; `History` com 100 snapshots `Arc<Song>`; `projectEpoch` zera o histórico ao trocar de música (como hoje).
- Export/import de arquivo: `.wadawko.json` (aceita `.taiko.json`), via diálogo nativo (`rfd`).

---

## 4. Estratégia de performance de UI no GPUI

### 4.1 Grade/timeline como Element customizado com culling

- Um `GridElement` por bloco de seção (ou um só para a música), implementando `Element` com `request_layout` (tamanho = `total_cols × cell_w × zoom`, alturas por linha), `prepaint` (calcula `col0..col1`, `row0..row1` visíveis a partir de `bounds` ∩ offset de scroll e **só** modela/shapeia texto dessas células; cache `HashMap<(sílaba, tamanho, bucket de zoom), ShapedLine>` — o vocabulário tem 20 sílabas + kiai, hit rate ~100 %) e `paint` (quads de fundo/linhas de tempo/seleção/segmentos multi-golpe, `ShapedLine::paint`, hint do dono, trilhos de pessoa, playhead). Hit-testing por aritmética (`(x − x0) / cell_w`) com `window.on_mouse_event` registrado no `paint`, sem um elemento por célula.
- Edição: **um único** input flutuante (`gpui-component::Input`) posicionado sobre a célula com foco; teclado (Tab/espaço/setas/Enter/Esc, `!`, Ctrl+C/X/V, Delete) movem a "célula-cursor" e o input. Isso elimina a maior parte das 2434 linhas do `Grid.svelte`, que existem por causa de 1 `<input>` por célula.
- Linhas de dinâmica, kiai e movimentos (seção 6) são faixas do mesmo Element, desenhadas no mesmo passe.
- Custo por frame: O(células visíveis) ≈ 8 linhas × 60 colunas ≈ 500 quads + 300 textos, trivial para 120 Hz. Música de 5000 eventos ou 50 000 não muda o custo.

### 4.2 Zoom e scroll suaves

- Estado próprio `{offset_x: f32, zoom: f32}` na view; `on_scroll_wheel` muda offset (Shift = horizontal) e Ctrl+roda muda zoom **ancorado no ponteiro**: `offset' = (offset + px) · z'/z − px`. Zoom contínuo (não só os 3 degraus de hoje), com snap em 0,5/1/2. Nenhuma re-layout da árvore: só muda a aritmética do `prepaint`.
- Suavização: interpolar offset/zoom em direção ao alvo a cada frame (`request_animation_frame`) com critério de parada; `follow()` do playback vira o mesmo mecanismo (alvo = playhead − 40 % da viewport), como o auto-scroll atual.
- Se o profile mostrar o layout do taffy (não o nosso paint) como gargalo, trocar o pin para `gpui-fast` (mesma API, cache de layout entre frames).

### 4.3 Playhead sem re-render pesado

- O playhead lê `sample_pos`/`stamp` dos atômicos e extrapola `t = pos + (now − stamp)·sr` (a granularidade do callback é 5–10 ms, insuficiente para 120 fps), clampando em `pos + buffer_len`. Converte para tick via `bpm` atual e dobra para o compasso-fonte (`transportSeconds`).
- Desenho em um **`PlayheadElement` irmão**, posicionado em absoluto sobre a grade, que é o único a pedir `request_animation_frame` enquanto `playing`. O `GridElement` não recalcula nada: seu `prepaint` compara `(song_version, viewport, zoom)` com o cache em `ElementState` e reutiliza as `ShapedLine`. Hipótese a validar no spike: GPUI reexecuta `render()` da view a cada frame, mas com o cache o custo fica sub-milissegundo.
- Highlight de célula/nota ao golpe (hoje `hitListener` + `Tone.Draw`) vem da fila `Event::HitFired{key, sample}`; a view agenda um flash com duração `min(400, max(140, 30000/bpm))` ms, como em `Score.svelte`.

### 4.4 Esteira, partitura e palco

- Esteira: `LaneElement` com `x = (sec_evento − t)·380 px/s`, culling por janela `[−90, w+48]` px, discos como quads/paths; pulso do tambor e as pétalas viram um sistema de partículas no mesmo canvas (hoje são nós DOM com Web Animations).
- Partitura: pinta via `Canvas2D` (seção 5) com culling por compasso visível; flash por golpe = recolorir o glifo da nota (mapa `key → índices`), não re-engravar.
- Palco dos bonecos: seção 6.6.

### 4.5 Texto e fontes

- Embutir Zen Kaku Gothic New (UI/sílabas), Shippori Mincho (marca) e Bravura (SMuFL), registradas no `TextSystem` na inicialização. Tamanhos fixos por nível de zoom (buckets) para o cache de shaping não explodir.

---

## 5. Notação musical (substituto do VexFlow)

O que a partitura atual exige (`score.ts`) é um **subconjunto pequeno**: pauta de 1 linha por tambor, cabeças normal/×, hastes para cima, bandeirolas/beams por tempo, pausas, quiáltera de 3, acento `>`, texto `sfz`, sílaba embaixo, kiai em itálico vermelho, cor do dono, barra de compasso conectando pautas, barra final, repetição (ritornello — hoje nem desenhado) e, no futuro, dinâmicas pp–ff e hairpins (já existem nos dados). Como todas as linhas compartilham a mesma grade de células, **não precisamos de um Formatter**: a posição x de cada golpe é `x_compasso + (célula + offset)·largura_célula` — exatamente a esteira, só que com glifos. O VexFlow só resolvia layout e desenho.

| Opção | Esforço | Qualidade | Observações |
|---|---|---|---|
| **A. Engraving próprio (subconjunto) com SMuFL/Bravura** | ~1,5–2 k linhas | Alta (glifos profissionais, OFL) | Glifos: `noteheadBlack` U+E0A4, `noteheadXBlack` U+E0A9, bandeirolas U+E240+, pausas U+E4E5–E4E8, acento U+E4A0, dinâmicas U+E520+, número de quiáltera U+E883, barras de repetição U+E040/E041. Beams, hastes, hairpins e colchetes são retângulos/paths. `bravura_metadata.json` dá as âncoras de haste. Mesmo código pinta na tela (GPUI), em SVG (impressão A4 paisagem, pedido antigo do `/planejamento/inicial.md`) e em PNG via o trait `Canvas2D`. |
| B. Gerar SVG por um crate | — | — | **Não existe** engraver de partitura em Rust. "Via crate" só seria: (B1) o nosso engraving emitindo SVG e rasterizando com `resvg` (GPUI já embute resvg para o elemento `svg`) — perde highlight por nota barato e vira a opção A com um passo a mais; (B2) embutir o VexFlow num runtime JS (QuickJS/Boa) → SVG → resvg: pesado, lento, frágil; (B3) Verovio (C++ LGPL, MEI) ou LilyPond CLI: mapeamento de percussão doloroso, só faria sentido para um "export de impressão de luxo" futuro. |
| C. Remover a feature | 0 | — | A partitura é read-only e secundária (serve a percussionistas de formação ocidental e impressão). Aceitável como estado temporário, não como destino. |

**Recomendação: A**, na Fase 5, escrita contra `wadawko-draw::Canvas2D` (que a seção 6 também usa). Se o esforço estourar, cair em C temporariamente e priorizar "imprimir grade" por SVG, que é trivial.

---

## 6. Bonecos animados (personagens 2D por Pessoa)

### 6.1 Objetivo e escopo

Uma nova view **"palco"** (aba ao lado de grade/partitura/esteira) com um personagem por `Person`, em pé no(s) instrumento(s) que toca, batendo **automaticamente** conforme as notas (mão D/E, couro × borda, dinâmica → amplitude) e executando **movimentos pré-programados** (poses, giros de bachi, kamae, saltos, kiai) numa lane de coreografia por pessoa — o estilo Sukeroku é performático (naname-dai, giros, poses), e a anotação de kata era o item sem dono do backlog. Tudo sincronizado ao relógio do áudio. Export opcional em vídeo. Estilo visual **original**.

Fora do escopo v1: IK, 3D, física de roupa, editor gráfico de keyframes (os movimentos são autorados em arquivos `.ron`), captura de movimento.

### 6.2 Modelo de dados (campos opcionais, `version: 1` mantido)

```rust
// wadawko-core::model
pub struct Person { /* ...atual... */ pub avatar: Option<Avatar> }
pub struct Avatar { pub body: BodyPreset /* slim|regular|stout */, pub height: f32 /* 0.85..1.15 */,
                    pub happi: Option<Color> /* default = person.color */, pub hachimaki: Option<Color>,
                    pub handedness: Hand /* R|L: mão forte, espelha a kamae */ }

pub struct Track { /* ...atual... */ pub stand: Option<Stand> }
pub enum Stand { Naname /* inclinado, Sukeroku — default p/ chu/nagado/hira */, Beta /* baixo, horizontal */,
                 Upright /* ōdaiko em pé, dois lados */, ShimeStand, Yatai /* sentado */, Handheld /* kane */ }

pub struct Song { /* ...atual... */ pub choreo: Option<Choreo> }
pub struct Choreo { pub moves: Vec<MoveDef> /* custom da música; builtins ficam em assets/moves */,
                    pub lanes: BTreeMap<PersonId, Vec<MoveEvent>> }
pub struct MoveEvent { pub id: String, pub at: GridPos { measure: u32, cell: u32 } /* alinhado à grade */,
                       pub len_cells: u32, pub move_id: String, pub mirror: bool, pub intensity: f32 /* 0.5..1.5 */ }

// wadawko-anim (também serializável, p/ MoveDef custom)
pub struct MoveDef { pub id: String, pub name: String, pub tags: Vec<Tag> /* pose, bachi, step, kiai */,
                     pub duration_beats: f32, pub loopable: bool, pub mirrorable: bool, pub locks_arms: bool,
                     pub keyframes: Vec<Keyframe> }
pub struct Keyframe { pub t: f32 /* 0..1 */, pub pose: Pose /* ângulos por osso + offset do root */, pub ease: Ease }
pub enum Ease { Linear, InOut, In, Out, Overshoot(f32), Anticipate(f32), Bounce }
```

Compatibilidade: música antiga sem `avatar/stand/choreo` renderiza com defaults (todo mundo em kamae no naname-dai, sem movimentos). Remover uma pessoa apaga a lane dela (`deletePerson`); `setSubdivision` reamostra `MoveEvent.at/len_cells` como faz com `cells`; `removeTrack` não mexe em coreografia (a lane é da pessoa, não da linha).

### 6.3 Cena e posicionamento

- Palco 2D em vista frontal levemente elevada (sem perspectiva real; profundidade por escala e ordem de desenho). Uma **estação** por `Track` (tambor + `Stand`), dispostas da esquerda para a direita na ordem de `song.tracks`; cada personagem fica na estação do seu instrumento. Pessoa com 2+ instrumentos: fica entre as estações e o corpo gira (`torso.yaw` em 2D = troca de lado/escala do braço) para a que vai tocar, com antecedência (regra de 6.4). Instrumento **compartilhado** (2 pessoas, `cellOwners`): um de cada lado do tambor (como ōdaiko) — o personagem que bate é o **dono da célula**, reaproveitando `cellOwnerOf`. Linha sem pessoa: tambor sem boneco (ou um "ayakashi" cinza, opcional).
- O tambor e o toque usam a semântica de cor do app: couro vermelho (pulsa no golpe `head`), fuchi azul (flash em `ka`). Dono mudo (`personAudible` false) → boneco translúcido, como as notas nas outras views.

### 6.4 Animação automática de golpes (com antecipação)

Derivada do `Schedule` compilado (mesmos eventos do áudio, então o contato é **por construção** o instante do golpe):

1. Para cada pessoa, lista de `Strike{tick, hand, zone, amp, station}` onde `amp = f(vel · dyn · mainMul)` em 0..1 (ghost tsu/ku ≈ 0.1; don ≈ 0.5; DON/acento ≈ 0.8; sfz/ff ≈ 1.0 → braço acima da cabeça, o grande arco Sukeroku; rim/ka ≈ 0.6·amp com a ponta do bachi apontada à borda; doron/karan = as duas mãos simétricas).
2. Perfil por golpe, em ticks → segundos pelo BPM corrente: **windup** começa em `t − W`, **contato** em `t` (keyframe exato, `Ease::In` no fim), **follow-through** até `t + R`, retorno à pose "pronto" da estação. `W = clamp(0.35·beat_dur·(0.6 + 0.4·amp), 60 ms, 350 ms)`, limitado a `0.8 · IOI` até o golpe anterior **da mesma mão** (rulos `dokodoko` encurtam e reduzem a amplitude automaticamente). `R ≈ 0.4·W`.
3. Como a lista é conhecida com antecedência, não há "previsão": a pose no instante `t` é uma função pura `pose(t) = idle ⊕ move_lane(t) ⊕ strikes(t)` — o windup do próximo golpe começa sozinho porque `t > t_hit − W`. Durante playback com trainer (BPM muda) `W` é recalculado por frame, já que tudo é em ticks.
4. Camadas e prioridade: braço que bate = camada de golpe (override) na janela `[t−W, t+R]`; o resto do corpo = lane de movimentos; se o movimento ativo tem `locks_arms` (giro de bachi), ele vence e o golpe é omitido visualmente (a música continua). Extras automáticos: leve inclinação do tronco em acentos, cabeça marcando o tempo (metrônomo interno), boca abre e cabeça sobe nos eventos de kiai (`kyaiEvents`, `!` = grito).
5. Preview de célula ao digitar (`engine.preview`) também dispara o golpe no boneco, com `W` mínimo.

### 6.5 Lane de movimentos, biblioteca e operações de grupo

- **Lane por pessoa** na grade (faixa colapsável, mesma mecânica da sub-linha de kiai), chips com o nome do movimento ocupando `len_cells` células; arrastar move, arrastar a borda redimensiona, duplo clique abre a biblioteca, Delete apaga, `M` espelha. Também visível como faixa fina no topo da view palco (contexto).
- **Biblioteca** (`assets/moves/*.ron`, hot reload em dev via `notify`): `kamae` (postura pronta por `Stand`), `furiage` (bachi erguidos acima da cabeça), `kime` (pose final), `bachi_twirl_r/l/both`, `step_l/r`, `turn_180`, `jump`, `crouch`, `bow`, `kiai_shout` (cabeça+braço), `rest`, `idle_breath` (loop). ~12–15 na v1, com preview em loop num mini-palco do painel. Movimentos custom por música = `Song.choreo.moves` (criados por duplicar + ajustar parâmetros; edição de keyframes só no arquivo, v1).
- **Operações** em `core::ops::choreo`: `copy_lane(from, to, range, offset_cells, mirror)` (cópia, **cânone** = cópia com offset, **espelho L/R** = troca ossos L↔R e nega componentes horizontais; só movimentos `mirrorable`), `shift(range, delta)`, `fill_pattern(range, move, every_n_cells)`. Undo/redo cobre tudo (são mutações de `Song`).

### 6.6 Rig 2D e renderização — comparação

Rig: 14 ossos em FK (`root/pelvis, torso, neck/head, upper_arm_{l,r}, forearm_{l,r}, hand_{l,r}+bachi, thigh_{l,r}, shin_{l,r}`), cada um com rotação, comprimento e uma **parte** (forma vetorial em espaço local). `Pose` = mapa osso → ângulo (+ offset/escala do root). Avaliação: `lerp` de ângulos por osso com easing por keyframe; blend entre camadas por peso com rampas de 80 ms. Transformações compostas 2D (`glam::Affine2`) → desenho.

| Abordagem | Prós | Contras | Veredito |
|---|---|---|---|
| **(a) Cut-out vetorial no canvas GPUI** — cada parte é um `Path` (bezier) preenchido + contorno, transformado pelo osso | Amplitude contínua (dinâmica), espelho trivial, zoom sem borrão, assets em texto (RON/SVG simples), ~15 paths/boneco × 8 pessoas ≈ 120 paths + tambores por frame: tessela em < 1 ms (verificar no spike) | Visual "recortado" (que é o estilo escolhido), sem texturas | **Recomendado v1** |
| (b) Sprite sheets (quadros rasterizados) | Desenho baratíssimo (`paint_image`), permite arte pintada | Cada pose × amplitude × stand precisa de quadros; sem interpolação livre; borra no zoom; produção de arte enorme; cânone/espelho ok, dinâmica não | Rejeitado, salvo atlas pequeno de **rostos/expressões** sobre o rig |
| (c) Superfície wgpu embutida | Liberdade total (skinning, shaders) | GPUI 0.3.x não expõe API pública para embutir superfície/textura externa no tree; caminhos são janela separada ou readback CPU → `RenderImage` → `paint_image` a cada frame (1080p RGBA a 60 fps ≈ 0,5 GB/s de upload) | Rejeitado para v1; reconsiderar só se formos a 3D |
| (d) Partes rasterizadas (PNG com alpha, pincel) posicionadas pelo esqueleto | Arte mais rica que (a), mesmo rig | Exige rotação de imagem no GPUI (`TransformationMatrix`) — **assumido, confirmar**; borra no zoom | Upgrade natural de (a) na v2, se a API existir |

O rig e a avaliação ficam em `wadawko-anim`, agnósticos de renderizador; o desenho passa por `Canvas2D`, o que dá de graça o backend tiny-skia para o export de vídeo (6.8) e a partitura (5).

### 6.7 Sincronização com o áudio

- O relógio da animação é **o do áudio**: `t_samples = sample_pos + (now − stamp)·sr`, extrapolado entre callbacks e clampado (4.3), somado ao `output_latency` do stream (cpal expõe quando o backend informa; senão, campo "atraso visual" em ms nas configurações, calibrável a olho com o metrônomo — o mesmo problema que jogos de ritmo resolvem com offset). Conversão samples → ticks pelo BPM corrente e dobra pro compasso-fonte, idêntica ao playhead.
- Logo, golpes, movimentos e o playhead da grade são **a mesma função de `t`**; o contato cai no mesmo frame em que o `Event::HitFired` chega (usado só para o pulso do tambor e, por segurança, para "agarrar" o contato se o frame se atrasou).
- Parado: `t` = `startCell` (os bonecos ficam na pose do instante, útil para posar ao editar a lane).

### 6.8 Export de vídeo (opcional)

Pipeline offline, sem depender do GPUI: (1) `offline::render` → WAV (Fase 2); (2) para cada frame a 60 fps, `t = frame/60` → avaliar cena → desenhar com o backend **tiny-skia** do `Canvas2D` em RGBA 1920×1080 (ou 1280×720); (3) `ffmpeg -f rawvideo -pix_fmt rgba -s WxH -r 60 -i - -i audio.wav -c:v libx264 -pix_fmt yuv420p -c:a aac out.mp4` via `std::process::Command` com stdin em pipe; (4) progresso e cancelamento na UI. `ffmpeg` é dependência externa detectada em runtime (`which ffmpeg`; mensagem clara se faltar) — o projeto já o usa em `scripts/`. Opções v1: só palco; v2: sobrepor a grade/sílabas rolando ("karaokê de kuchi shōga").

### 6.9 Direção de arte (original)

- Linguagem: **papel recortado + tinta sumi** — formas chapadas com contorno irregular de pincel (dois paths deslocados fazem a variação de espessura), paleta restrita aos tokens do app (`--accent` vermelho = couro/ação, `--accent-2` azul = fuchi, neutros frios; a cor da pessoa vai no **happi**; hachimaki vermelho/azul conforme a mão forte), bachi em madeira crua, tambores com corpo escuro e pele clara, fundo "papel" com grão sutil no tema claro e lousa no escuro. Proporções estilizadas mas adultas (cabeça ~1/5 da altura, mãos/bachi grandes para a leitura do gesto), rosto mínimo (dois traços + boca que abre no kiai). Silhueta primeiro: cada pose precisa ler bem em preto chapado.
- Não-objetivos explícitos: nenhum mascote; **não** reproduzir personagens, proporções ou acabamento de jogos existentes (Taiko no Tatsujin/Don-chan, Rhythm Heaven, etc.); nada de estilo "chibi kawaii". A customização do usuário é por cor/altura/corpo, o estilo é fixo.
- Referência de movimento (não de visual): vídeos do Oedo Sukeroku Taiko para timing de furiage, giros e kime; o Kuchishoga Periodic Table para vocabulário de movimentos.

---

## 7. O que acontece com `server/`

`server/dbPlugin.ts` existe apenas porque o browser não abre SQLite: é um middleware do Vite que expõe `data/wadawko.db` como `/api/kv`, com `src/lib/db.ts` espelhando em localStorage e enfileirando gravações offline. No app nativo nada disso é necessário: `wadawko-store` abre o banco com `rusqlite`, na **mesma tabela e chaves**, então:

1. Durante a transição (Fases 1–6), os dois apps podem apontar para o mesmo `data/wadawko.db` (um de cada vez — WAL permite leitura concorrente, mas evitar edições simultâneas). O `wadawko-cli` serve para conferir que o Rust lê/escreve o que o web app gravou.
2. No cutover (Fase 7): apagar `server/`, `src/lib/db.ts`, o plugin de `vite.config.ts` e todo o stack web (ou arquivar num branch `web-legacy`). O banco migra para `$XDG_DATA_HOME/wadawko/` (cópia na primeira execução, com checkpoint do WAL). `data/backup-localstorage-*.json` vira fixture de teste e fonte do `import-backup`.
3. O "backend" que o `/planejamento/inicial.md` §6.1 considerou para render de MP3 continua desnecessário: o render offline é local e mais rápido que tempo real.

---

## 8. Fases e entregáveis


### 8.0 Resultado parcial do spike (2026-11-10, `spike/`)

Máquina: i5-1135G7 + Iris Xe (Mesa/Vulkan), CachyOS, Wayland, monitor 60 Hz, **load average ~10 durante os testes** (outros jobs rodando). Build `--release`, `--bench 15 --mute` (follow contínuo = grade re-renderiza todo frame, zoom oscilando 3× a cada 1,5 s), 5000 eventos.

| Medida | Resultado | Critério |
|---|---|---|
| Intervalo de frame (Wayland, com bonecos ligados) | p50 16,67 · p95 17,21 · p99 17,47 ms → 59,9 fps, preso no vsync de 60 Hz | ≤ 16 ms ✅ (120 Hz não medível neste monitor) |
| Paint da grade (CPU, por frame) | p50 1,3–1,7 · p95 2,3–2,7 · max 4,5 ms | cabe em 8,3 ms ✅ |
| Paint dos 8 bonecos (~130 paths) | p50 1,2 · p95 1,8 ms | ≤ 2 ms ✅ (feature adiada) |
| Áudio 512 frames | 0 callbacks atrasados, 0 erros, DSP max 0,35 ms, latência reportada 21–32 ms | ✅ |
| Áudio 256 frames | 0 atrasados, DSP max 0,19 ms, latência 15,9 ms | ✅ |
| X11 (XWayland) | janela abre, áudio/paint ok; **fps não medido** | ⏳ |

Pendente (precisa de alguém olhando a tela): rodadas seguintes deram 0 frames — o compositor não entrega frame callbacks para janela fora de vista, então o fps de X11/256 e a sincronia áudio×visual "a olho" ficam para rodar manualmente. Playback longo de 10 min também.

**Atualização (mesmo dia): UX da web no protótipo.** `spike/` agora reproduz o layout do `App.svelte`/`Grid.svelte`: NavRail, topbar (BPM, reto/swing, loop, metrônomo, count-in, treino, tocar, tema), abas grade/partitura/esteira, ContextToolbar (editar/selecionar, colcheia/semicolcheia/fusa, + seção, instrumento, + linha, zoom), seções (ritornello, recolher, ± compasso, clonar, remover), régua `1 e 2 e`, dinâmica por compasso, linhas com M/S/×, barras de golpe couro/fuchi, paleta flutuante de sílabas, painel MÚSICAS e status bar; tema claro/escuro com os tokens de `app.css`; fontes Zen Kaku Gothic New + Shippori Mincho (OFL) embutidas. Lê `data/wadawko.db` **somente leitura** (edições ficam em memória). Funciona: tocar/parar, seek pela régua, follow horizontal/vertical, swing, loop, metrônomo, mute/solo, ritornellos, dinâmica (clique cicla pp→ff), digitar/apagar sílabas, seleção retangular. Só visual por enquanto: ⚙/f/★, count-in, treino, hairpin, exportar/importar/mp3, páginas Pessoas/Mixer/Kit, partitura/esteira. Áudio: porte do `audio.ts` em `spike/src/sound.rs` — 74 samples por instrumento/zona com camadas de dinâmica (vel·√dyn) e round robin, TUNING, velGain, humanização (ganho ±12 %, pitch ±1 %, tempo ±6 ms), herança de dinâmica + sforzando, cauda máxima (TAIL), choke (couro abafa couro+borda), EQ por instrumento (biquads RBJ) + filtro de timbre/pitch/ganho/decay das InstrumentDefs (builtins, presets e customizadas da música), volume e pan por linha, master −6 dB + soft clip. Faltam: hairpins, mainMul, kiai, compressor do master. `--render "Título" saida.wav` gera WAV offline com o mesmo motor.

**Latência e destaque de notas (mesmo dia).** O grafo do PipeWire aqui roda com quantum 1024 (21 ms) e o cliente ALSA somava buffer de 512 → som ~40–60 ms atrás do comando, e o visual (alinhado pela latência que o cpal informa, que subestima) ficava adiantado. Agora: `PIPEWIRE_LATENCY=256/48000` definido no início do `main` (o grafo cai para quantum 256; 0 xruns novos em 8 s tocando, máquina com load ~4) + buffer padrão 256 → latência informada 31,8 → 15,9 ms. Células acendem quando a nota soa (vermelho couro / azul borda, reacende em cada golpe de doko/kara, apaga até a próxima), calculado do relógio audível (`audible_tick` − latência − offset); status bar mostra "♪ tocando · c2 · 2.1". Offset de sincronia ajustável (Configurações → Sincronia, ou `:offset 40`), salvo em settings.json.

**Export/import (mesmo dia).** Botões do painel + paleta + `Ctrl+E` (JSON), `Ctrl+Shift+E` (MP3), `Ctrl+O` (importar), "wav". JSON = objeto Song v1 da web (`<título>.taiko.json`), escrito a partir do JSON original de cada música/faixa/compasso/seção e sobrescrevendo só o que o app nativo edita — pessoas, kiai, donos de célula, hairpins etc. passam intactos (papparapa, mikazuchi e Renshū exportados sem edição saem idênticos ao banco). Ao mudar a subdivisão, `cellOwners`/`kyai` do compasso são descartados (índices não valem mais). MP3/WAV: render offline com o mesmo motor do playback em thread de fundo, normalização de pico 0,98 como a web, MP3 192 kbps via `ffmpeg` (libmp3lame) com título nos metadados. Diálogos de arquivo pelo xdg-desktop-portal (`prompt_for_new_path`/`prompt_for_paths`); sem portal, o export salva direto em ~/Downloads. CLI: `--render "Título" saida.(mp3|wav|taiko.json)`.

Descobertas de API que corrigem §2:
- O pacote de plataforma **existe**: `gpui-pre-platform 0.3.12`, puxado pela fachada **`gpui-kit = "=0.9.1"`** (`gpui_kit::application()`, `gpui_kit::open_window`, `gpui_kit::init`). Usar `gpui-kit` como única dependência, não `gpui-pre` + `gpui-component` soltos.
- `ScrollWheelEvent`/`on_scroll_wheel` → renomeados para `ScrollEvent`/`on_scroll` (aliases deprecados).
- `AnyView/Entity::cached(style)` funciona como esperado: grade em cache, playhead num view irmão sem cache.
- `canvas(prepaint, paint)`, `paint_quad`/`fill`/`outline`, `PathBuilder` + `paint_path`, `text_system().shape_line` + `ShapedLine::paint`, `with_content_mask`, `request_animation_frame`, `track_focus` + `on_key_down`: todos presentes no pin.
- O `Input` do gpui-component 0.9 não foi usado (renomeado/reestruturado; edição feita com teclado direto na grade).
- Build limpa de release: ~19 min nesta máquina carregada; incremental ~5–25 s.

### Fase 0 — Spike descartável (go/no-go do GPUI)

Crate `spike/` fora do workspace final; o código é jogado fora, só as conclusões ficam.

Entregáveis:
- Janela GPUI (`gpui-pre =0.3.12`, mesmo pin do `gpui-component 0.9.1`), tema claro/escuro, fonte Zen Kaku embutida; abre em **Wayland e X11** (`WAYLAND_DISPLAY` setado/não).
- `TimelineElement` customizado com **5000 eventos** (8 linhas × 625 células, sílabas aleatórias do `VOCABULARY`, segmentos multi-golpe), culling por viewport, scroll por roda, zoom contínuo Ctrl+roda ancorado no ponteiro, seleção por arrasto (hit-test aritmético), um `Input` flutuante sobre a célula focada.
- Thread de áudio com `cpal` (256–512 frames, 48 kHz), pool fixo de 32 vozes, sem alocação no callback; toca metrônomo sintético + `chu-head-1-0.ogg` (decodificado com symphonia) seguindo um `Schedule` em ticks; comandos Play/Stop/Seek/SetBpm por `rtrb`; `sample_pos` em `AtomicU64` + stamp; contador de xruns.
- Playhead desenhado por Element irmão com `request_animation_frame`, extrapolação do relógio; cursor acompanha o som.
- Mini-teste de bonecos: 120 paths preenchidos + contorno re-tesselados por frame (8 figuras de 15 partes) girando — mede o custo de 6.6(a).
- Instrumentação: fps e p50/p95/p99 do frame time na tela (`tracing` + histograma), medição de latência clique-visual (gravação da tela com áudio, ou a olho).

Go/no-go (critérios objetivos, na máquina do dev):
- **GO** se: p95 do frame time ≤ 8,3 ms (120 Hz) — mínimo aceitável ≤ 16 ms (60 Hz) — com 5000 eventos, playhead animando, em Wayland **e** X11; zoom contínuo por 10 s sem stutter; 10 min de playback sem xrun a 512 frames (e idealmente 256); sincronia áudio/visual ≤ 1 frame a olho; as APIs necessárias existem no pin (Element customizado ou canvas, paint_quad/path, shaping de texto, scroll wheel, animation frame, foco de teclado, input de texto); 120 paths/frame ≤ 2 ms; build incremental ≤ 10 s; binário roda numa instalação limpa (só Mesa/Vulkan).
- **NO-GO** se: não abre janela em um dos backends; frame time > 16 ms mesmo com culling; API de pintura/texto obriga a reimplementar coisas básicas; crash recorrente; o pin de `gpui-pre` quebra em duas semanas seguidas sem alternativa. Então: seção 9, plano B.

### Fase 1 — Fundação (core + store + CLI)
Workspace; `wadawko-core` portado de `types.ts`, `vocabulary.ts`, `phrase.ts`, `timeline.ts`, `kyai.ts` (parte de texto) e das operações do `store.svelte.ts`; testes de tabela com as 20 sílabas, `dooon/dokon/DOKO/don!`, `playOrder` com ritornello e região, `dynamicsCurve` com hairpin e override por instrumento, `setSubdivision`, clone/move de seção; **fixtures = os 4 projetos do `data/backup-localstorage-2026-10-02.json` + demo** com teste de round-trip JSON byte-a-byte-equivalente (chaves e valores). `wadawko-store` abrindo `data/wadawko.db`. `wadawko-cli list|export|import-backup`.

### Fase 2 — Áudio e export
`wadawko-samples` (embed + decode), `wadawko-audio` completo (3.2–3.3): paridade com `audio.ts` item a item (checklist: layers/RR, velGain, humanização, choke, TAIL, EQ, tone/overtones/decay/pitch/gain por zona, canais/barramentos, mudo/solo, swing, loop/região, seek, count-in, trainer, metrônomo, kiai, previews, master), render offline + `lookahead_limit`, WAV/MP3, `wadawko-cli render` para comparar A/B com o MP3 do web app. Teste de regressão: render de uma fixture com seed fixo → hash estável.

### Fase 3 — Shell e grade (o maior bloco)
`wadawko-app`: tema (tokens de `app.css`), shell (NavRail, topbar com transporte, painel direito de músicas, status bar), `GridElement` com tudo do `Grid.svelte` (seções, timeline/playhead/seek, células, segmentos, dinâmicas, kiai, popover de linha, modo seleção e clipboard, trilhos de pessoa, caneta), paleta flutuante com hover-preview de frases, `ContextToolbar`, command palette, undo/redo, autosave, import/export, atalhos. Marco: **editar e tocar a música "Renshū" do banco atual ponta a ponta**.

### Fase 4 — Views secundárias
Esteira (canvas, partículas de pétalas), Mixer, Pessoas, Kit (sliders por zona + preview), easter egg do fue. Marco: paridade funcional com o web app exceto partitura.

### Fase 5 — Notação
`wadawko-draw::Canvas2D` (backends GPUI + SVG + tiny-skia), `wadawko-notation` (seção 5), view partitura com highlight por golpe e auto-scroll, export SVG/PNG e impressão A4 paisagem.

### Fase 6 — Bonecos (ADIADA)
- **6a** rig, biblioteca inicial (kamae/furiage/kime/idle), estações por `Stand`, golpes automáticos com antecipação, view palco sincronizada, `Avatar` em Pessoas. Marco: "Renshū" tocada por 3 bonecos com contato no tempo.
- **6b** lane de movimentos na grade, biblioteca completa com preview, operações copiar/espelhar/cânone, `MoveDef` custom, hot reload dos `.ron`.
- **6c** export de vídeo (tiny-skia + ffmpeg) com progresso.
- Em paralelo: iteração de arte (silhuetas, timing dos movimentos) com vídeos de referência.

### Fase 7 — Cutover e distribuição
Migração do banco para XDG, remoção do stack web (branch `web-legacy`), créditos dos samples na UI (obrigação CC BY-SA), AppImage + Flatpak (Linux), CI com builds Linux e best-effort macOS/Windows (gpui-component anuncia os três), documentação de uso.

Dependências: 0 → 1 → 2 → 3 → 4; 5 depende de 1 e 3 (Canvas2D pode começar na 3); 6 depende de 2 (schedule), 3 (lane na grade) e do `Canvas2D` (5 ou feito antes); 7 depende de tudo.

---

## 9. Riscos e mitigações

| Risco | Impacto | Mitigação |
|---|---|---|
| **Churn de API do GPUI** (pré-1.0; `gpui-pre` muda toda semana; docs rasas; `gpui_platform` citado na doc nem existe publicado) | Quebra de build, retrabalho | Pin exato `=0.3.12` casado com `gpui-component`; atualizar só em "dia de pin" com changelog; isolar GPUI em `wadawko-app` + backend GPUI do `Canvas2D` (todo o resto não sabe que GPUI existe); ler o código-fonte do Zed como documentação; spike valida a superfície de API que usamos |
| **Linux Wayland/X11** (blade/Vulkan, scaling fracionário, clipboard, teclado) | Janela não abre, texto borrado, atalhos falham | Spike testa os dois backends; exigir Mesa/Vulkan; flag `--x11`/`--wayland`; evitar features exóticas (sem IME, sem drag-and-drop nativo na v1); flatpak com runtime conhecido |
| **Latência/xruns no PipeWire** (cpal usa ALSA → `pipewire-alsa`) | Estalos, sincronia ruim | Buffer 512 por padrão (256 opcional), callback sem alocação, schedule com lookahead (jitter da GUI não importa), contador de xruns visível; alternativa `cpal` feature `jack` (PipeWire-JACK) se ALSA-plug decepcionar; a sincronia visual usa `output_latency` + offset calibrável |
| **Esforço da notação** (sem engraver em Rust) | Fase 5 estoura | Subconjunto definido por `score.ts` (sem Formatter); Bravura + metadata; fallback C temporário (sem partitura) sem bloquear as outras fases |
| **Bonecos: qualidade de animação é trabalho de design** | Movimento duro ou "errado" para Sukeroku | Hot reload de `.ron`, mini-palco de preview, referência em vídeo, iterar com o grupo; v1 FK 2D só |
| Bonecos: sincronia visual com granularidade de buffer | Contato "atrasado" | Extrapolação + clamp + offset de latência (6.7); `HitFired` como segurança |
| Bonecos: custo de paths por frame | Queda de fps com 8 pessoas | Medido no spike; cache de tesselação por parte; LOD (sem contorno em zoom pequeno) |
| `mp3lame-encoder` é LGPL | Obrigações de licença | Link dinâmico ou aceitar LGPL no componente; alternativa ffmpeg CLI |
| Samples CC BY-SA embutidos no binário | O pacote de samples permanece CC BY-SA | Tela de créditos (porte do `StatusBar`/`CREDITS.md`); permitir pasta de samples do próprio grupo |
| Reescrita longa sem valor intermediário | Desânimo, duas bases vivas | Fases com marcos usáveis (1: CLI lê o banco; 2: render por CLI; 3: editar/tocar); os dois apps compartilham o mesmo banco até o cutover |
| Um dev só, GPUI pouco documentado | Estimativas furam | Ranges nas estimativas; spike antes de qualquer compromisso |

**Plano B se o spike reprovar o GPUI:** manter **todos** os crates exceto `wadawko-app` e trocar o backend de UI:
- **iced** (wgpu, `canvas` widget com geometria cacheada e `Program` para interação — encaixa na grade, esteira e palco; winit cobre Wayland/X11; docs maduras; widgets de formulário suficientes para mixer/kit): primeira escolha. O backend `Canvas2D` para iced é um mapeamento direto (`Frame::fill/stroke/fill_text`).
- **Slint** (declarativo `.slint`, backends Skia/femtovg): bom para painéis, fraco para canvas custom intensivo (seria `Path` elements ou imagem de CPU); segunda escolha.
- **egui**: imediato, `Painter` trivial para canvas, visual menos polido; opção pragmática se a prioridade for chegar rápido.
O plano B custa ~2 semanas a mais na Fase 3 e nada nas outras.

---

## 10. Estimativa de tamanho por fase

Para uma pessoa experiente em Rust, dedicação integral (dobrar para meio período). LOC é Rust novo, sem testes.

| Fase | Semanas | LOC aprox. | Observações |
|---|---|---|---|
| 0 Spike | 1–2 | ~1,5 k (descartável) | Inclui ler fonte do GPUI |
| 1 Fundação | 2–3 | ~3 k + 1,5 k testes | Porte de ~1,8 k linhas TS de domínio |
| 2 Áudio + export | 3–4 | ~3,5 k | Porte de `audio.ts`/`kyai.ts` (1,6 k TS) + DSP + offline |
| 3 Shell + grade | 4–6 | ~6 k | O `Grid.svelte` (2,4 k) encolhe, mas shell/commands/history/inputs somam |
| 4 Views secundárias | 2–3 | ~2,5 k | Esteira ~600, Mixer/Pessoas/Kit ~1,5 k |
| 5 Notação | 3–4 | ~2,5 k (+ `Canvas2D` 0,8 k com 3 backends) | |
| 6a Bonecos: rig + golpes + palco | 2–3 | ~2,5 k | + ~1 semana de arte/timing |
| 6b Lane + biblioteca + operações | 2–3 | ~2 k | 12–15 movimentos em RON |
| 6c Vídeo | 1 | ~0,6 k | ffmpeg externo |
| 7 Cutover + distribuição | 1–2 | ~0,5 k + CI | |
| **Total** | **21–31 semanas** | **~25 k** | Sem bonecos: 16–24 semanas |

---

## 11. Referências verificadas em 2026-11-10

- crates.io API: `gpui` 0.2.2 (2025-10-22); `gpui-pre` 0.3.12 e `gpui-fast` 0.1.14 (2026-11-09); `gpui-component`/`gpui-kit` 0.9.1 (2026-11-09, depende de `gpui-pre =0.3.12`); `gpui_platform` inexistente (404); `cpal` 0.18.2; `kira` 0.12.6; `mp3lame-encoder` 0.3.0 (LGPL-3.0).
- docs.rs/gpui 0.2.2: `canvas`, `PaintQuad`, `Path/PathBuilder`, `uniform_list`/`list`, `ScrollWheelEvent`; docs "ainda sendo melhoradas".
- github.com/longbridge/gpui-kit (ex-gpui-component): 60+ componentes, Linux/macOS/Windows, Apache-2.0.
- github.com/longbridge/gpui-fast: GPUI extraído do monorepo com cache de layout entre frames.
- Zed: crates/gpui no monorepo (fonte = documentação).
- Fontes: Bravura (SMuFL, OFL), Zen Kaku Gothic New e Shippori Mincho (OFL).
- Código-base deste plano: `src/lib/{types,phrase,vocabulary,timeline,audio,kyai,score,lane,store.svelte,db,exportAudio}.ts`, `src/lib/{Grid,Score,Lane,MixerPage,PeoplePanel,KitPage,RightPanel,ContextToolbar,CommandPalette}.svelte`, `src/App.svelte`, `server/dbPlugin.ts`, `scripts/extract-samples.py`, `public/samples/CREDITS.md`, `data/`.

---

### Critical Files for Implementation
- src/lib/audio.ts — engine Tone.js a portar 1:1 (layers/RR, choke, EQ, TAIL, canais/barramentos, transport, trainer, renderOffline, lookaheadLimit)
- src/lib/store.svelte.ts — todas as operações de domínio e o esquema de persistência (`wadawko:workspace`, `wadawko:project:<id>`)
- src/lib/types.ts — modelo `Song` v1 que o serde deve espelhar byte a byte
- src/lib/Grid.svelte — comportamento completo da grade (teclado, seleção, seções, dinâmicas, kiai, playhead) a reproduzir no `GridElement`
- src/lib/score.ts — define o subconjunto exato de engraving que a notação própria precisa cobrir (junto com `phrase.ts`/`timeline.ts` para o schedule e os bonecos)