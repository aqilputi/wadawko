// Gera tests/fixtures/golden.json rodando o TypeScript ORIGINAL do app web
// (src/lib/*.ts) — a fonte da verdade do porte. O app web saiu do `main`; ele fica
// na tag `web-final`. Para regerar, da raiz deste repositório:
//
//   git worktree add .web web-final && (cd .web && npm ci)
//   node crates/wadawko-core/tests/golden/gen.mjs        # ou WADAWKO_WEB=<checkout> node …
//   git worktree remove .web
//
// O store é empacotado com esbuild (o do checkout web) com stubs de `$state`
// (identidade), `./db` (memória) e `tone`/`vexflow` (vazios); `Date.now` é fixado pra os
// ids serem determinísticos (os testes Rust usam `ids::set_deterministic` com o mesmo valor).
import { readFileSync, writeFileSync, mkdtempSync, existsSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { join, dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const root = resolve(here, '../../../..')
const web = resolve(process.env.WADAWKO_WEB ?? join(root, '.web'))
if (!existsSync(join(web, 'src/lib/store.svelte.ts')) || !existsSync(join(web, 'node_modules/esbuild'))) {
  console.error(`checkout do app web não encontrado em ${web} (precisa de src/lib e node_modules: veja o topo deste arquivo)`)
  process.exit(1)
}
const { build } = createRequire(join(web, 'package.json'))('esbuild')
const NOW = 1790000000000

const stub = {
  name: 'stubs',
  setup(b) {
    b.onResolve({ filter: /^(tone|vexflow)$/ }, (a) => ({ path: a.path, namespace: 'stub' }))
    b.onResolve({ filter: /\/db$/ }, (a) => ({ path: 'db', namespace: 'stub' }))
    b.onLoad({ filter: /.*/, namespace: 'stub' }, (a) =>
      a.path === 'db'
        ? { contents: 'const m=new Map();export const loadDb=async()=>{};export const dbGet=(k)=>m.get(k)??null;export const dbSet=(k,v)=>{m.set(k,v)};export const dbRemove=(k)=>{m.delete(k)};export const dbStatus={online:false}' }
        : { contents: 'module.exports = new Proxy({}, { get: () => function () {} })', loader: 'js' },
    )
  },
}
const outdir = mkdtempSync(join(process.env.WADAWKO_TMP ?? tmpdir(), 'wadawko-golden-'))
const outfile = join(outdir, 'bundle.mjs')
// reexporta o TS original do app web
const entry = ['store.svelte:S', 'phrase', 'timeline', 'kyai', 'vocabulary', 'types']
  .map((m) => { const [f, n] = m.split(':'); return `export * as ${n ?? f} from './src/lib/${f}'` })
  .concat(["export { demoSong } from './src/lib/demo'"])
  .join('\n')
await build({ stdin: { contents: entry, resolveDir: web, loader: 'ts', sourcefile: 'entry.ts' }, bundle: true, format: 'esm', platform: 'node', outfile, plugins: [stub], logLevel: 'error' })

globalThis.$state = (x) => x
const realNow = Date.now
Date.now = () => NOW
const { S, phrase, timeline, kyai, vocabulary, types, demoSong } = await import(pathToFileURL(outfile).href)
// audio.ts não carrega fora do browser (Tone.js no topo do módulo): extrai só o
// texto de swingWarp, tira as anotações de tipo e avalia.
const swingSrc = readFileSync(join(web, 'src/lib/audio.ts'), 'utf8').match(/export function (swingWarp\([\s\S]*?\n\})\n/)[1]
const swingWarp = new Function(`return function ${swingSrc.replaceAll(': number', '')}`)()
void realNow

// ───────── util ─────────
function mulberry32(a) {
  return () => {
    a |= 0
    a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}
const canon = (v) => (Array.isArray(v) ? v.map(canon) : v && typeof v === 'object' ? Object.fromEntries(Object.keys(v).sort().filter((k) => v[k] !== undefined).map((k) => [k, canon(v[k])])) : v)
function fnv(s) {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return h >>> 0
}
const songHash = () => fnv(JSON.stringify(canon(JSON.parse(JSON.stringify(S.store.song)))))
const clone = (x) => JSON.parse(JSON.stringify(x))

const backup = JSON.parse(readFileSync(join(here, '../fixtures/backup-localstorage-2026-10-02.json'), 'utf8'))
const BASES = {
  demo: demoSong(),
  big: JSON.parse(backup['wadawko:project:p-muog58u1-1']),
  pap: JSON.parse(backup['wadawko:project:p-muplamne-1']),
}

// ───────── execução de operações ─────────
const pick = (arr, i) => (arr && arr.length ? arr[i % arr.length].id : 'nope')
function res(a) {
  const song = S.store.song
  if (a && typeof a === 'object' && !Array.isArray(a)) {
    if ('$t' in a) return pick(song.tracks, a.$t)
    if ('$s' in a) return pick(song.sections, a.$s)
    if ('$p' in a) return pick(song.people, a.$p)
    if ('$i' in a) return pick(song.instruments, a.$i)
  }
  return a
}
function normRet(r) {
  if (r && typeof r === 'object' && 'written' in r) return { written: r.written.map((p) => [p.mi, p.ci]), next: r.next ? [r.next.mi, r.next.ci] : null }
  if (r && typeof r === 'object' && 'id' in r) return r.id
  if (typeof r === 'string') return r
  return null
}
function exec(op) {
  const [name, ...raw] = op
  const a = raw.map(res)
  const song = S.store.song
  switch (name) {
    case 'setSelected': S.store.selectedSections = raw[0].map(res); return null
    case 'setPen': S.store.activePersonId = a[0]; return null
    case 'setPersonFlags': { const p = song.people?.find((x) => x.id === a[0]); if (p) { p.muted = a[1]; p.solo = a[2] } return null }
    case 'setLegacyMain': delete song.mainTrackIds; song.mainTrackId = a[0]; return null
    case 'setSwing': song.swing = a[0]; return null
    case 'setCellsOwner': return normRet(S.setCellsOwner(raw[0].map(([t, mi, ci]) => ({ trackId: res(t), mi, ci })), a[1]))
    case 'updateInstrumentZone': { const patch = { ...a[2] }; if (patch.tone === null) patch.tone = undefined; return normRet(S.updateInstrumentZone(a[0], a[1], patch)) }
    case 'appendSection': case 'addPerson': return normRet(S[name](a[0] ?? undefined))
    case 'addInstrument': return normRet(S.addInstrument(a[0], a[1] ?? undefined))
    case 'addMeasure': case 'removeMeasure': return normRet(S[name](a[0] ?? undefined))
    default: return normRet(S[name](...a))
  }
}

function queries() {
  const song = S.store.song
  const { playOrder, dynamicsCurve, mainTracks, mainMul } = timeline
  const order = playOrder(song)
  const region = S.selectedRegion()
  const orderR = playOrder(song, region)
  const sample = (ord, tid) => { const f = dynamicsCurve(song, ord, tid); return ord.flatMap((_, u) => [f(u, 0), f(u, 0.5)]) }
  const evs = kyai.kyaiEvents(song, orderR, (f) => swingWarp(f, song.swing ?? 0))
  return {
    ranges: S.sectionRanges().map((r) => [r.id, r.start, r.end + 1]),
    order,
    region: region ? [region.startBar, region.endBar] : null,
    orderR,
    dyn: sample(order, undefined),
    dynTracks: song.tracks.map((t) => sample(orderR, t.id)),
    mainTracks: mainTracks(song),
    mainMul: song.tracks.map((t) => mainMul(song, t.id)),
    rails: S.railsForTracks().map((r) => [r.person.id, r.rows]),
    audible: (song.people ?? []).map((p) => S.personAudible(p.id)),
    owners: song.tracks.map((t) => song.measures.flatMap((m, mi) => Array.from({ length: types.cellsPerMeasure(m) }, (_, ci) => S.cellOwner(t.id, mi, ci)))),
    inst: song.tracks.map((t) => [S.resolveInstrument(t).id, S.instrumentLabel(t)]),
    totalCols: S.totalCols(),
    hasKyai: song.tracks.map((t) => !!S.trackHasKyai(t.id)),
    hasDyn: song.tracks.map((t) => S.trackHasDynamics(t.id)),
    kyai: evs.map((e) => [e.track.id, e.u, e.mi, e.ci, e.beat, e.frac, e.beats, e.syl, e.idx, e.n, e.shout, Number.isFinite(e.gapBeats) ? e.gapBeats : null, kyai.kyaiSeed(e.track, song.measures[e.mi], e.ci)]),
  }
}

const cases = []
function runCase(name, base, ops, { full = true } = {}) {
  S.store.song = clone(BASES[base])
  S.store.selectedSections = []
  S.store.activePersonId = null
  const rets = []
  const hashes = []
  for (const op of ops) {
    rets.push(exec(op))
    hashes.push(songHash())
  }
  cases.push({ name, base, ops, rets, hashes, song: full ? clone(S.store.song) : null, queries: queries() })
}

const t = (i) => ({ $t: i })
const s = (i) => ({ $s: i })
const p = (i) => ({ $p: i })
const inst = (i) => ({ $i: i })

// ───────── casos escritos à mão ─────────
runCase('base-demo', 'demo', [])
runCase('base-big', 'big', [], { full: false })
runCase('base-pap', 'pap', [], { full: false })
runCase('big-region', 'big', [['setSelected', [s(1), s(3)]]], { full: false })
runCase('big-region-solo', 'big', [['setSelected', [s(13)]]], { full: false })
runCase('pap-region', 'pap', [['setSelected', [s(1)]]], { full: false })

runCase('sections-demo', 'demo', [
  ['appendSection', null],
  ['appendSection', 'B'],
  ['appendSection', null],
  ['setSectionRepeat', s(0), 3],
  ['setSectionRepeat', s(1), 40],
  ['setSectionRepeat', s(2), 0],
  ['setCell', t(1), 4, 0, 'don'],
  ['cloneSection', s(0)],
  ['cloneSection', s(2)],
  ['cloneSection', 'nope'],
  ['renameSection', s(1), 'Intro bis'],
  ['moveSection', s(0), 3],
  ['moveSection', s(4), 0],
  ['moveSection', s(2), 99],
  ['removeSection', s(1)],
  ['setSelected', [s(0), s(2)]],
  ['addMeasure', null],
  ['addMeasure', s(0)],
  ['removeMeasure', null],
  ['removeMeasure', s(3)],
  ['removeMeasure', s(3)],
  ['setSectionRepeat', s(0), 1],
])
runCase('sections-big', 'big', [
  ['moveSection', s(17), 0],
  ['moveSection', s(3), 9],
  ['cloneSection', s(1)],
  ['cloneSection', s(12)],
  ['removeSection', s(5)],
  ['addMeasure', s(2)],
  ['removeMeasure', s(7)],
  ['setSelected', [s(4), s(9)]],
  ['addMeasure', null],
  ['ensureCols', 1200],
])
runCase('remove-all-sections', 'demo', [
  ['appendSection', null],
  ['removeSection', s(0)],
  ['appendSection', null],
  ['appendSection', null],
  ['removeSection', s(0)],
  ['removeSection', s(0)],
  ['removeMeasure', null],
])
runCase('pre-section-measures', 'demo', [
  ['appendSection', null],
  ['appendSection', null],
  ['removeSection', s(0)],
  ['appendSection', 'C'],
  ['moveSection', s(1), 0],
  ['cloneSection', s(0)],
  ['setSectionRepeat', s(0), 2],
  ['setSelected', [s(1)]],
])
runCase('dynamics', 'demo', [
  ['addMeasure', null], ['addMeasure', null], ['addMeasure', null], ['addMeasure', null],
  ['setMeasureDynamic', 0, 'p', null],
  ['toggleMeasureHairpin', 1, 'cresc', null],
  ['toggleMeasureHairpin', 2, 'cresc', null],
  ['setMeasureDynamic', 3, 'ff', null],
  ['toggleMeasureHairpin', 4, 'dim', null],
  ['toggleMeasureHairpin', 6, 'cresc', null],
  ['setMeasureDynamic', 1, 'f', t(1)],
  ['toggleMeasureHairpin', 2, 'dim', t(1)],
  ['toggleMeasureHairpin', 3, 'dim', t(1)],
  ['toggleMeasureHairpin', 3, 'dim', t(1)],
  ['setMeasureDynamic', 5, 'pp', t(2)],
  ['setMeasureDynamic', 5, null, t(2)],
  ['setMeasureDynamic', 6, null, t(2)],
  ['toggleMeasureHairpin', 7, 'cresc', t(0)],
  ['setMeasureDynamic', 99, 'f', null],
  ['appendSection', null], ['setSectionRepeat', s(0), 2],
])
runCase('dynamics-big', 'big', [
  ['toggleMeasureHairpin', 0, 'cresc', null],
  ['setMeasureDynamic', 9, 'mp', t(0)],
  ['toggleMeasureHairpin', 10, 'dim', t(0)],
  ['toggleMeasureHairpin', 11, 'dim', t(0)],
  ['toggleMeasureHairpin', 130, 'cresc', null],
  ['setSelected', [s(1), s(2)]],
], { full: false })
runCase('cells', 'demo', [
  ['setCell', t(0), 0, 0, 'DON'],
  ['setCell', t(0), 0, 1, null],
  ['setCell', 'ghost-track', 1, 3, 'ka'],
  ['setCell', t(1), 9, 0, 'don'],
  ['setCell', t(1), 3, 11, 'don'],
  ['setCellCol', t(2), 17, 'kara'],
  ['setCellCol', t(2), 999, 'kara'],
  ['fillSequential', t(1), 0, 6, ['don', 'doko', 'ka', 'su']],
  ['fillSequential', t(1), 3, 6, ['don', 'doko', 'ka', 'su']],
  ['fillSequential', t(1), 0, 99, ['don']],
  ['fillPhraseBeats', t(0), 1, 3, ['don', 'doko', 'don', 'don']],
  ['fillPhraseBeats', t(0), 3, 5, ['DON', 'tsuku', 'DON', 'tsuku']],
  ['fillPhraseBeats', t(2), 2, 7, ['dokodoko', 'dokodoko', 'dokodoko', 'dokodoko', 'don']],
  ['fillKyai', t(1), 0, 0, ['so', 're', 'so', 're!']],
  ['fillKyai', t(1), 1, 2, ['sei', 'ya!']],
  ['fillKyai', t(0), 3, 6, ['so', 'ri', 'ya!']],
  ['setKyai', t(0), 3, 6, null],
  ['setKyai', t(0), 3, 7, null],
  ['setKyai', t(2), 2, 0, 'ha!'],
  ['setKyai', t(2), 2, 0, null],
  ['setSwing', 0.6],
  ['appendSection', null], ['setSectionRepeat', s(0), 2],
])
for (const sub of [1, 2, 3, 4, 8]) {
  runCase(`subdivision-${sub}`, 'demo', [
    ['fillKyai', t(1), 0, 0, ['so', 're', 'so', 're!']],
    ['fillKyai', t(0), 3, 5, ['so', 'ri', 'ya!']],
    ['setMeasureDynamic', 1, 'f', null],
    ['setSubdivision', sub],
  ])
  runCase(`subdivision-pap-${sub}`, 'pap', [['setSubdivision', sub], ['setSubdivision', 2]])
}
runCase('subdivision-chain', 'demo', [['setSubdivision', 4], ['fillPhraseBeats', t(1), 0, 5, ['don', 'doko', 'don', 'don']], ['fillSequential', t(1), 2, 1, ['ka', 'ra', 'ka', 'ra', 'kara']], ['setSubdivision', 8], ['setSubdivision', 3], ['setSubdivision', 2], ['setSubdivision', 1], ['setSubdivision', 4]])
runCase('subdivision-big', 'big', [['setSubdivision', 4], ['setSubdivision', 3]])
runCase('people', 'demo', [
  ['addPerson', null],
  ['addPerson', 'Ana'],
  ['addPerson', null],
  ['toggleTrackPerson', t(1), p(0)],
  ['setCell', t(1), 0, 1, 'don'],
  ['toggleTrackPerson', t(1), p(1)],
  ['setCell', t(1), 0, 2, 'ka'],
  ['setPen', p(1)],
  ['setCell', t(1), 0, 3, 'ka'],
  ['fillSequential', t(1), 1, 0, ['don', 'don']],
  ['setPen', p(2)],
  ['setCell', t(1), 0, 4, 'ka'],
  ['setCell', t(1), 0, 3, null],
  ['fillPhraseBeats', t(1), 2, 0, ['don', 'doko']],
  ['assignCellOwner', t(1), 3, 0, p(1)],
  ['assignCellOwner', t(1), 3, 2, p(2)],
  ['assignCellOwner', t(1), 3, 4, null],
  ['setCellsOwner', [[t(1), 3, 6], [t(1), 2, 1], [t(0), 0, 0]], p(0)],
  ['toggleTrackPerson', t(0), p(2)],
  ['toggleTrackPerson', t(2), p(0)],
  ['toggleTrackPerson', t(2), p(2)],
  ['renamePerson', p(0), 'Bia'],
  ['setPersonColor', p(0), '#123456'],
  ['setPersonFlags', p(1), true, false],
  ['setPersonFlags', p(2), false, true],
  ['fillKyai', t(1), 3, 0, ['so', 're!']],
  ['fillKyai', t(2), 3, 0, ['ha!']],
  ['fillKyai', t(0), 3, 0, ['hai!']],
])
runCase('people-delete', 'demo', [
  ['addPerson', null], ['addPerson', null], ['addPerson', null],
  ['toggleTrackPerson', t(1), p(0)], ['toggleTrackPerson', t(1), p(1)], ['toggleTrackPerson', t(1), p(2)],
  ['setPen', p(1)],
  ['fillSequential', t(1), 0, 0, ['don', 'don', 'don', 'don']],
  ['setPen', p(2)],
  ['fillSequential', t(1), 1, 0, ['don', 'don', 'don', 'don']],
  ['setSubdivision', 2],
  ['deletePerson', p(1)],
  ['setCell', t(1), 2, 0, 'ka'],
  ['toggleTrackPerson', t(1), p(0)],
  ['deletePerson', p(0)], ['deletePerson', p(0)], ['deletePerson', 'nope'],
  ['addPerson', null],
])
runCase('tracks', 'pap', [
  ['addTrack', 'shime'],
  ['addTrack', 'shime'],
  ['addTrack', 'preset-odaiko-profundo'],
  ['addTrack', inst(0)],
  ['addTrack', inst(0)],
  ['addTrack', 'inexistente'],
  ['renameTrack', t(1), 'Novo nome'],
  ['toggleMainTrack', t(0)],
  ['toggleMainTrack', t(2)],
  ['toggleMainTrack', t(0)],
  ['setTrackInstrument', t(0), 'kane'],
  ['setTrackInstrument', t(1), inst(2)],
  ['setTrackInstrument', t(1), 'inexistente'],
  ['moveTrack', t(0), -1],
  ['moveTrack', t(0), 1],
  ['moveTrack', t(3), 1],
  ['setMeasureDynamic', 0, 'f', t(2)],
  ['fillKyai', t(2), 0, 0, ['ha!']],
  ['removeTrack', t(2)],
  ['removeTrack', 'nope'],
  ['deleteInstrument', inst(0)],
])
runCase('legacy-main', 'demo', [['setLegacyMain', t(1)], ['addMeasure', null]])
runCase('legacy-main-toggle', 'demo', [['setLegacyMain', t(1)], ['toggleMainTrack', t(2)]])
runCase('legacy-main-remove', 'demo', [['setLegacyMain', t(1)], ['removeTrack', t(0)]])
runCase('main-all', 'demo', [['toggleMainTrack', t(0)], ['toggleMainTrack', t(1)], ['toggleMainTrack', t(2)]])
runCase('kit', 'big', [
  ['addInstrument', 'odaiko', null],
  ['addInstrument', 'kane', 'Sino'],
  ['updateInstrumentZone', inst(4), 'head', { pitch: 3, decay: 0.25 }],
  ['updateInstrumentZone', inst(4), 'rim', { tone: { type: 'lowpass', cutoff: 1200, q: 1 } }],
  ['updateInstrumentZone', inst(0), 'head', { tone: null }],
  ['updateInstrumentZone', inst(1), 'rim', { gain: -1.5, overtones: 0.35 }],
  ['updateInstrumentZone', 'shime', 'head', { pitch: 3 }],
  ['renameInstrument', inst(5), 'Kane novo'],
  ['duplicateInstrument', 'preset-nagado-anelado'],
  ['duplicateInstrument', inst(0)],
  ['duplicateInstrument', 'nope'],
  ['setTrackInstrument', t(0), inst(6)],
  ['deleteInstrument', inst(6)],
  ['deleteInstrument', inst(0)],
], { full: true })

// ───────── fuzz: sequências aleatórias de operações ─────────
const SYLS = ['don', 'DON', 'doko', 'ka', 'kara', 'su', 'tsuku', 'don!', 'dokodoko', 'ten', 'xx', '']
const KY = ['so', 're!', 'sei', 'ya!', 'ha!', '']
const DYNS = ['pp', 'p', 'mp', 'mf', 'f', 'ff', null]
const BASES_I = ['shime', 'chu', 'hira', 'nagado', 'onagado', 'okedo', 'odaiko', 'kane']
function fuzzOps(rand, n) {
  const ri = (k) => Math.floor(rand() * k)
  const one = (arr) => arr[ri(arr.length)]
  const syl = () => (rand() < 0.2 ? null : one(SYLS))
  const maybe = (v) => (rand() < 0.5 ? v : null)
  const gens = [
    () => ['appendSection', maybe('X' + ri(9))],
    () => ['cloneSection', s(ri(8))],
    () => ['moveSection', s(ri(8)), ri(8)],
    () => ['renameSection', s(ri(8)), 'N' + ri(9)],
    () => ['setSectionRepeat', s(ri(8)), ri(20)],
    () => ['removeSection', s(ri(8))],
    () => ['setSelected', Array.from({ length: ri(3) }, () => s(ri(8)))],
    () => ['setPen', maybe(p(ri(4)))],
    () => ['setMeasureDynamic', ri(8), one(DYNS), maybe(t(ri(5)))],
    () => ['toggleMeasureHairpin', ri(8), one(['cresc', 'dim']), maybe(t(ri(5)))],
    () => ['addMeasure', maybe(s(ri(8)))],
    () => ['removeMeasure', maybe(s(ri(8)))],
    () => ['ensureCols', ri(80)],
    () => ['setSubdivision', one([1, 2, 2, 3, 4, 4, 8])],
    () => ['setCell', t(ri(5)), ri(8), ri(9), syl()],
    () => ['setCell', t(ri(5)), ri(8), ri(9), syl()],
    () => ['setCellCol', t(ri(5)), ri(70), syl()],
    () => ['setKyai', t(ri(5)), ri(8), ri(9), rand() < 0.3 ? null : one(KY)],
    () => ['fillSequential', t(ri(5)), ri(8), ri(9), Array.from({ length: ri(12) }, () => one(SYLS))],
    () => ['fillPhraseBeats', t(ri(5)), ri(8), ri(9), Array.from({ length: ri(7) }, () => one(SYLS))],
    () => ['fillKyai', t(ri(5)), ri(8), ri(9), Array.from({ length: ri(6) }, () => one(KY))],
    () => ['addTrack', rand() < 0.3 ? inst(ri(3)) : one([...BASES_I, 'preset-chu-grave', 'zzz'])],
    () => ['removeTrack', t(ri(5))],
    () => ['toggleMainTrack', t(ri(5))],
    () => ['renameTrack', t(ri(5)), 'T' + ri(9)],
    () => ['setTrackInstrument', t(ri(5)), rand() < 0.3 ? inst(ri(3)) : one(BASES_I)],
    () => ['moveTrack', t(ri(5)), one([-1, 1])],
    () => ['addPerson', maybe('P' + ri(9))],
    () => ['renamePerson', p(ri(4)), 'Q' + ri(9)],
    () => ['setPersonColor', p(ri(4)), '#00000' + ri(9)],
    () => ['deletePerson', p(ri(4))],
    () => ['toggleTrackPerson', t(ri(5)), p(ri(4))],
    () => ['toggleTrackPerson', t(ri(5)), p(ri(4))],
    () => ['toggleTrackPerson', t(ri(5)), p(ri(4))],
    () => ['assignCellOwner', t(ri(5)), ri(8), ri(9), maybe(p(ri(4)))],
    () => ['setCellsOwner', Array.from({ length: ri(4) }, () => [t(ri(5)), ri(8), ri(9)]), maybe(p(ri(4)))],
    () => ['setPersonFlags', p(ri(4)), rand() < 0.3, rand() < 0.3],
    () => ['addInstrument', one(BASES_I), maybe('I' + ri(9))],
    () => ['updateInstrumentZone', inst(ri(3)), one(['head', 'rim']), one([{ pitch: ri(12) - 6 }, { gain: -ri(5) / 2, decay: ri(11) / 10 }, { tone: { type: one(['lowpass', 'highpass', 'bandpass']), cutoff: 100 + 50 * ri(50), q: (1 + ri(30)) / 10 } }, { tone: null }, { overtones: ri(21) / 20 }])],
    () => ['renameInstrument', inst(ri(3)), 'R' + ri(9)],
    () => ['duplicateInstrument', rand() < 0.5 ? inst(ri(3)) : one(['chu', 'preset-okedo-brilhante', 'zzz'])],
    () => ['deleteInstrument', inst(ri(3))],
    () => ['setSwing', ri(11) / 10],
  ]
  return Array.from({ length: n }, () => one(gens)())
}
const fuzzRand = mulberry32(20261112)
for (let k = 0; k < 60; k++) runCase(`fuzz-${k}`, k % 3 === 2 ? 'pap' : 'demo', fuzzOps(fuzzRand, 80), { full: k < 6 })

// ───────── frase: splitWord / resolveCell / parseWords ─────────
const keys = Object.keys(vocabulary.VOCABULARY)
const words = new Set()
const cap = (w) => w[0].toUpperCase() + w.slice(1)
const stretch = (w, i, n) => w.slice(0, i + 1) + w[i].repeat(n) + w.slice(i + 1)
for (const k of keys) {
  for (const v of [k, k.toLowerCase(), k.toUpperCase(), cap(k.toLowerCase())]) {
    words.add(v)
    words.add(v + '!')
    for (let i = 0; i < v.length; i++) for (const n of [1, 2]) words.add(stretch(v, i, n))
  }
  for (const k2 of keys) {
    words.add(k + k2)
    words.add((k + k2).toLowerCase())
    words.add((k + k2).toUpperCase())
    words.add(k.toUpperCase() + k2.toLowerCase())
    words.add(k.toLowerCase() + k2.toUpperCase() + '!')
  }
}
const wr = mulberry32(7)
for (let n = 0; n < 500; n++) {
  let w = ''
  const parts = 1 + Math.floor(wr() * 4)
  for (let i = 0; i < parts; i++) {
    let k = keys[Math.floor(wr() * keys.length)]
    if (wr() < 0.3) k = k.toUpperCase()
    if (wr() < 0.3) { const i2 = Math.floor(wr() * k.length); k = stretch(k, i2, 1 + Math.floor(wr() * 3)) }
    if (wr() < 0.05) k = k.slice(0, -1)
    w += k
  }
  if (wr() < 0.2) w += '!'
  words.add(w)
}
for (const w of ['', '!', '!!', 'don!!', 'x', 'dokon', 'dooon', 'DOOON', 'doooko', 'DOKO', 'Don', 'dOn', 'dokodoko', 'dondon', 'dok', 'dokok', 'kkara', 'ttsu', 'tsuu', 'suuu', 'donn', 'doron', 'dorondon', 'karankaran', 'dôn', 'ＤＯＮ', 'don1', 'do-ko', 'İ', 'ſu', 'KARAN', 'Karan', 'teketen', 'TEKEten', 'kon!', 'KOn', 'ko!', 'k!', 'su!', 'SU', 'TSUKU', 'tsukutsu', 'dododo', 'kokoko', 'dokokon', 'dokokara', 'tsutsu']) words.add(w)
const enc = (w) => {
  const sp = phrase.splitWord(w)
  const rc = phrase.resolveCell(w)
  return [
    w,
    sp ? sp.map((x) => [x.syl, x.stretch, x.accent]) : null,
    rc ? { sfz: rc.sfz, hits: rc.hits.map((h) => [h.offset, h.zone, h.vel, h.hand, h.accent, !!h.sfz]), parts: rc.parts.map((pt) => [pt.syl, pt.hits.map((h) => [h.offset, h.zone, h.vel, h.hand, h.accent, !!h.sfz])]) } : null,
    phrase.isSfz(w),
    phrase.stripSfz(w),
    phrase.toggleSfz(w),
  ]
}
const texts = ['don doko don don', '  DON,tsuku;DON|tsuku  ', 'don xx', '', '   ', 'dokodoko dokodoko!', 'don! ! ka', 'don\tka\nra', 'don ka', 'don　ka﻿ra', ...vocabulary.PHRASES.map((x) => x.phrase)]

// ───────── kiai ─────────
const kr = mulberry32(99)
const ALPHA = 'aeiouaeiouknsrhytmc-!n '
const kyTexts = new Set([...kyai.KYAI_PRESETS, '', ' ', '!', 'a', 'n', 'nn', 'an', 'ana', 'anna', 'kyo', 'ryia', 'hyia!', 'sokka', 'chotto', 'shu', 'tsu', 'sēi', 'so-re', 'soーre!', 'SEIYA!', 'yoi yoi, yoi!', 'ou', 'oui', 'aia', 'ei!!', 'k', 'st', 'yi', 'yia', 'ya', 'y', 'wasshoi!', 'don', 'x1', 'a-', '-a', 'nya', 'kan-', 'hon', 'honya', 'hona', 'souya', 'soiya!', 'yoisho!', 'dokkoisho', 'iyaa!', 'yo-o!', 'a!b', 'ii', 'iii', 'ooi'])
for (let n = 0; n < 3000; n++) {
  let w = ''
  const len = 1 + Math.floor(kr() * 9)
  for (let i = 0; i < len; i++) w += ALPHA[Math.floor(kr() * ALPHA.length)]
  kyTexts.add(w)
}
const seeds = ['shime', 'chu1', 'odaiko', 'person-muogk8wy-1', 'person-x-2', '', 'Ōdaiko', 'あ', '𝄞', 'a', 'b', 'p-1', 'nagado-mf3k2-7']
const voices = [undefined, {}, { pitch: 3 }, { vary: 0.1, gain: -3 }, { pitch: -2.5, vary: 1 }]
const profiles = []
for (const sd of seeds) for (const v of voices) {
  const track = { id: 't', name: 't', instrument: 'chu', muted: false, solo: false, ...(v ? { kyai: v } : {}) }
  const pr = kyai.kyaiProfile(sd, track)
  profiles.push([sd, v ?? null, [pr.pitch, pr.speed, pr.vary, pr.grit, pr.breath, pr.range]])
}
const contours = []
for (let n = 1; n <= 20; n++) for (let idx = 0; idx < n; idx++) for (const sh of [false, true]) contours.push([idx, n, sh, kyai.contour(idx, n, sh)])
// kyaiNote com Math.random determinístico
const notes = []
{
  const nr = mulberry32(5)
  const realRandom = Math.random
  for (let k = 0; k < 200; k++) {
    const n = 1 + Math.floor(nr() * 5)
    const idx = Math.floor(nr() * n)
    const shout = nr() < 0.5
    const gap = nr() < 0.2 ? null : 0.05 + nr() * 0.8
    const sd = seeds[Math.floor(nr() * seeds.length)]
    const v = voices[Math.floor(nr() * voices.length)]
    const track = { id: 't', name: 't', instrument: 'chu', muted: false, solo: false, ...(v ? { kyai: v } : {}) }
    const prof = kyai.kyaiProfile(sd, track)
    const rs = [nr(), nr(), nr(), nr()]
    let i = 0
    Math.random = () => rs[i++]
    const note = kyai.kyaiNote({ syl: 'ya!', idx, n, shout }, gap ?? Infinity, prof)
    Math.random = realRandom
    notes.push([idx, n, shout, gap, sd, v ?? null, rs, [note.st0, note.st1, note.dur, note.gain]])
  }
}
const swing = []
for (const sw of [0, 0.25, 0.5, 1, -1]) for (const f of [0, 0.125, 0.25, 0.5, 0.625, 0.75, 0.999, 1]) swing.push([f, sw, swingWarp(f, sw)])

const out = {
  now: NOW,
  vocabulary: Object.entries(vocabulary.VOCABULARY).map(([k, v]) => [k, v.hits.map((h) => [h.offset, h.zone, h.vel, h.hand]), v.hint]),
  palette: vocabulary.PALETTE_GROUPS,
  phrases: vocabulary.PHRASES,
  builtins: types.BUILTIN_INSTRUMENTS,
  instrumentLabels: types.INSTRUMENTS,
  dynamicLevels: types.DYNAMIC_LEVELS,
  personColors: types.PERSON_COLORS,
  kyaiPresets: kyai.KYAI_PRESETS,
  demo: demoSong(),
  words: [...words].map(enc),
  parseWords: texts.map((x) => [x, phrase.parseWords(x)]),
  splitKyai: [...kyTexts].map((x) => [x, kyai.splitKyai(x)]),
  profiles,
  contours,
  notes,
  swing,
  cases,
}
const dest = resolve(here, '../fixtures/golden.json')
writeFileSync(dest, JSON.stringify(out))
console.log(`golden: ${cases.length} casos, ${out.words.length} palavras, ${out.splitKyai.length} kiai → ${dest} (${(JSON.stringify(out).length / 1024).toFixed(0)} KB)`)
