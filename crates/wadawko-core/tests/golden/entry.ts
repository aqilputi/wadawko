// Ponto de entrada empacotado pelo gen.mjs: reexporta o TS original do app web
// pra gerar os casos "golden" que os testes Rust conferem.
export * as S from '../../../../src/lib/store.svelte'
export * as phrase from '../../../../src/lib/phrase'
export * as timeline from '../../../../src/lib/timeline'
export * as kyai from '../../../../src/lib/kyai'
export * as vocabulary from '../../../../src/lib/vocabulary'
export * as types from '../../../../src/lib/types'
export { demoSong } from '../../../../src/lib/demo'
