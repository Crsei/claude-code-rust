import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { useChatStore } from './lib/store'
import './index.css'

if (import.meta.env.DEV) {
  // Dev-only escape hatch so we can drive the store from the browser
  // console or automated preview tools without plumbing new hooks.
  (window as unknown as { __CC_STORE__?: typeof useChatStore }).__CC_STORE__ =
    useChatStore
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
