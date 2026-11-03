import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"
import { compression } from "vite-plugin-compression2"

// The Rust service embeds web/dist and serves the .br/.gz siblings when the
// browser accepts them.
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    compression({
      algorithms: ["brotliCompress", "gzip"],
      include: /\.(js|css|html|svg|json|woff2?|txt)$/,
      threshold: 512,
      skipIfLargerOrEqual: true,
    }),
  ],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://localhost:8740",
        changeOrigin: true,
        ws: true,
      },
    },
  },
  build: {
    target: "es2022",
    chunkSizeWarningLimit: 600,
    rolldownOptions: {
      output: {
        advancedChunks: {
          groups: [
            { name: "react", test: /node_modules[\\/](react|react-dom|scheduler|react-router)[\\/]/ },
            { name: "codemirror", test: /node_modules[\\/](@codemirror|@lezer|crelt|style-mod|w3c-keyname)[\\/]/ },
          ],
        },
      },
    },
  },
})
