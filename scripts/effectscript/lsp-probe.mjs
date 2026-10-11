import { spawn } from "node:child_process"
import { readFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
import path from "node:path"

const [tsgo, file, line, character] = process.argv.slice(2)
if (!tsgo || !file) {
  console.log("usage: node scripts/effectscript/lsp-probe.mjs <tsgo> <file> [line character]")
  process.exit(2)
}
const absolute = path.resolve(file)
const uri = pathToFileURL(absolute).href
const root = pathToFileURL(path.dirname(absolute)).href
const server = spawn(tsgo, ["--lsp", "--stdio"], { stdio: ["pipe", "pipe", "inherit"] })

let buffer = Buffer.alloc(0)
const pending = new Map()
let nextId = 1
const diagnostics = []

server.stdout.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk])
  for (;;) {
    const headerEnd = buffer.indexOf("\r\n\r\n")
    if (headerEnd < 0) return
    const length = Number(/Content-Length: (\d+)/i.exec(buffer.subarray(0, headerEnd).toString())[1])
    if (buffer.length < headerEnd + 4 + length) return
    const message = JSON.parse(buffer.subarray(headerEnd + 4, headerEnd + 4 + length).toString())
    buffer = buffer.subarray(headerEnd + 4 + length)
    if (message.id !== undefined && pending.has(message.id)) {
      pending.get(message.id)(message)
      pending.delete(message.id)
    } else if (message.id !== undefined && message.method) {
      send({ jsonrpc: "2.0", id: message.id, result: null })
    } else if (message.method === "textDocument/publishDiagnostics" && message.params.uri === uri) {
      diagnostics.push(message.params.diagnostics)
    }
  }
})

function send(message) {
  const body = JSON.stringify(message)
  server.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`)
}

function request(method, params) {
  const id = nextId++
  send({ jsonrpc: "2.0", id, method, params })
  return new Promise((resolve) => pending.set(id, resolve))
}

const notify = (method, params) => send({ jsonrpc: "2.0", method, params })
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

await request("initialize", {
  processId: process.pid,
  rootUri: root,
  workspaceFolders: [{ uri: root, name: "probe" }],
  capabilities: { textDocument: { publishDiagnostics: {}, hover: { contentFormat: ["plaintext"] } } }
})
notify("initialized", {})
notify("textDocument/didOpen", {
  textDocument: { uri, languageId: process.env.LANGUAGE_ID ?? "typescript", version: 1, text: readFileSync(absolute, "utf8") }
})
const pulled = await request("textDocument/diagnostic", { textDocument: { uri } })
console.log("pull diagnostics:", JSON.stringify(pulled.result ?? pulled.error))
if (line !== undefined) {
  const hover = await request("textDocument/hover", {
    textDocument: { uri },
    position: { line: Number(line), character: Number(character) }
  })
  console.log("hover:", JSON.stringify(hover.result?.contents ?? hover.error))
}
await sleep(500)
console.log("pushed diagnostics:", JSON.stringify(diagnostics.at(-1) ?? null))
await request("shutdown", null)
notify("exit", null)
