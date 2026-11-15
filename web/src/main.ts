import { createApp } from 'vue'
import App from './App.vue'
import { initializeRuntime } from './lib/runtimeBridge'
import './vendor/ace/system.css'
import './style.css'
initializeRuntime().then(() => createApp(App).mount('#app')).catch(error => {
  const root = document.querySelector('#app')
  if (root) root.textContent = `Runtime initialization failed: ${String(error)}. Reload to retry.`
})
