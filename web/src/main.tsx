import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import { App } from "./App.tsx"
import "./index.css"

async function boot() {
  // The mock API exists only in `npm run dev:mock`; with VITE_MOCK unset the
  // condition is false at build time and the import is dropped.
  if (import.meta.env.VITE_MOCK === "1") {
    await import("@/mock")
  }
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  )
}

void boot()
