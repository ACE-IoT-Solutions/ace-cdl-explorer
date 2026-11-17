import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { quizResultsEndpointForBuild } from './build-config.mjs'

export default defineConfig(({ command }) => ({
  plugins: [vue()],
  base: './',
  define: { __QUIZ_RESULTS_ENDPOINT__: JSON.stringify(quizResultsEndpointForBuild(command, process.env)) },
  worker: { format: 'es' },
  build: { target: 'es2022' },
}))
