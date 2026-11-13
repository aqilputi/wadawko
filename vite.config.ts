import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { dbPlugin } from './server/dbPlugin'

export default defineConfig({
  plugins: [svelte(), dbPlugin()],
  base: './',
  server: { host: true, watch: { ignored: ['**/data/**'] } },
  preview: { host: true },
})
