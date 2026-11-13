import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

const app = mount(App, { target: document.getElementById('app')! })

if (import.meta.env.DEV) {
  Promise.all([import('./lib/phrase'), import('./lib/vocabulary')]).then(([phrase, vocab]) => {
    ;(window as unknown as Record<string, unknown>).__debug = { ...phrase, ...vocab }
  })
}

export default app
