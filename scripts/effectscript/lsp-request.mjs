import { spawn } from "node:child_process"
import { readFileSync } from "node:fs"
import { pathToFileURL } from "node:url"
import path from "node:path"

const [tsgo, file, method, paramsJson = "{}"] = process.argv.slice(2)
if (!tsgo || !file || !method) {
  console.log("usage: node scripts/effectscript/lsp-request.mjs <tsgo> <file> <method> [extra params json]")
  process.exit(2)
}
const absolute = path.resolve(file)
const uri = pathToFileURL(absolute).href
const server = spawn(tsgo, ["--lsp", "--stdio"], { stdio: ["pipe", "pipe", "inherit"] })
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
const send = (message) => {
  const body = JSON.stringify(message)
  server.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`)
}
const request = (name, params) => {
  const id = nextId++
  send({ jsonrpc: "2.0", id, method: name, params })
  return new Promise((resolve) => pending.set(id, resolve))
}
await request("initialize", {
  processId: process.pid,
  rootUri: pathToFileURL(path.dirname(absolute)).href,
  capabilities: { textDocument: { hover: { contentFormat: ["plaintext"] } } }
})
send({ jsonrpc: "2.0", method: "initialized", params: {} })
send({
  jsonrpc: "2.0",
  method: "textDocument/didOpen",
  params: {
    textDocument: {
      uri,
      languageId: absolute.endsWith(".ef") ? "effectscript" : "typescript",
      version: 1,
      text: readFileSync(absolute, "utf8")
    }
  }
})
const response = await request(method, { textDocument: { uri }, ...JSON.parse(paramsJson) })
console.log(JSON.stringify(response.result ?? response.error, null, 1))
server.kill()
