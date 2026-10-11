import { spawn } from "node:child_process"
import { readFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
import path from "node:path"

const [tsgo, file, step = "2"] = process.argv.slice(2)
if (!tsgo || !file) {
  console.log("usage: node scripts/effectscript/lsp-sweep.mjs <tsgo> <file> [column step]")
  process.exit(2)
}
const absolute = path.resolve(file)
const uri = pathToFileURL(absolute).href
const root = pathToFileURL(path.dirname(absolute)).href
const text = readFileSync(absolute, "utf8")
const lines = text.split("\n")
const languageId = absolute.endsWith(".ef") ? "effectscript" : "typescript"

const server = spawn(tsgo, ["--lsp", "--stdio"], { stdio: ["pipe", "pipe", "pipe"] })
server.stderr.resume()
let buffer = Buffer.alloc(0)
const pending = new Map()
let nextId = 1
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
    }
  }
})
server.on("exit", (code) => {
  for (const resolve of pending.values()) resolve({ error: { message: `server exited (${code})` } })
  pending.clear()
})

function send(message) {
  const body = JSON.stringify(message)
  server.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`)
}

function request(method, params) {
  const id = nextId++
  send({ jsonrpc: "2.0", id, method, params })
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      pending.delete(id)
      resolve({ error: { message: "timed out after 5s" } })
    }, 5000)
    pending.set(id, (message) => {
      clearTimeout(timer)
      resolve(message)
    })
  })
}

const notify = (method, params) => send({ jsonrpc: "2.0", method, params })

await request("initialize", {
  processId: process.pid,
  rootUri: root,
  workspaceFolders: [{ uri: root, name: "sweep" }],
  capabilities: {
    textDocument: {
      hover: { contentFormat: ["plaintext"] },
      completion: { completionItem: { snippetSupport: true } },
      semanticTokens: {
        requests: { full: true },
        tokenTypes: ["class", "function", "variable", "parameter", "property", "type", "interface"],
        tokenModifiers: ["declaration", "readonly"],
        formats: ["relative"]
      },
      documentSymbol: { hierarchicalDocumentSymbolSupport: true },
      foldingRange: {},
      inlayHint: {}
    }
  }
})
notify("initialized", {})
notify("textDocument/didOpen", { textDocument: { uri, languageId, version: 1, text } })

const doc = { textDocument: { uri } }
const failures = []
let hung = false
async function check(method, params, where = "") {
  if (hung) return
  const response = await request(method, params)
  if (response.error && /InternalError|timed out|exited/.test(response.error.message)) {
    failures.push(`${method}${where}: ${response.error.message}`)
    if (/timed out|exited/.test(response.error.message)) hung = true
  }
}

const whole = { start: { line: 0, character: 0 }, end: { line: lines.length, character: 0 } }
for (const [method, params] of [
  ["textDocument/diagnostic", doc],
  ["textDocument/foldingRange", doc],
  ["textDocument/documentSymbol", doc],
  ["textDocument/semanticTokens/full", doc],
  ["textDocument/formatting", { ...doc, options: { tabSize: 2, insertSpaces: true } }],
  ["textDocument/inlayHint", { ...doc, range: whole }],
  ["textDocument/codeLens", doc]
]) {
  await check(method, params)
}

const positional = [
  "textDocument/hover",
  "textDocument/definition",
  "textDocument/typeDefinition",
  "textDocument/references",
  "textDocument/documentHighlight",
  "textDocument/signatureHelp",
  "textDocument/completion",
  "textDocument/selectionRange",
  "textDocument/prepareRename",
  "textDocument/codeAction"
]
let positions = 0
for (let line = 0; line < lines.length && !hung; line++) {
  for (let character = 0; character <= lines[line].length && !hung; character += Number(step)) {
    positions++
    const position = { line, character }
    for (const method of positional) {
      const params =
        method === "textDocument/references"
          ? { ...doc, position, context: { includeDeclaration: true } }
          : method === "textDocument/selectionRange"
            ? { ...doc, positions: [position] }
            : method === "textDocument/codeAction"
              ? { ...doc, range: { start: position, end: position }, context: { diagnostics: [] } }
              : { ...doc, position }
      await check(method, params, ` at ${line + 1}:${character + 1}`)
    }
  }
}

server.kill()
const kinds = [...new Set(failures.map((failure) => failure.replace(/ at \d+:\d+/, "")))]
console.log(JSON.stringify({ file, positions, failures: failures.length, hung, kinds, first: failures.slice(0, 8) }, null, 2))
process.exit(failures.length ? 1 : 0)
