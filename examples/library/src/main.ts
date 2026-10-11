import { startServer } from "./server.js"
import { makeAppLayer, seedBooks } from "./services.js"

const port = Number(process.env.PORT ?? 3000)
const { url } = await startServer(makeAppLayer({ seed: seedBooks, echo: true }), port)

console.log(`library API on ${url}`)
console.log(`  GET  ${url}/books`)
console.log(`  POST ${url}/books              {"title": "...", "author": "..."}`)
console.log(`  POST ${url}/books/b1/borrow    {"member": "ada"}`)
console.log(`  POST ${url}/books/b1/return`)
console.log(`  GET  ${url}/audit`)
