import { createServer, type IncomingMessage, type Server } from "node:http"
import type { AddressInfo } from "node:net"
import { type Layer, ManagedRuntime } from "effect"
import { handle } from "./http.js"
import type { AuditLog, BookRepo, Ids } from "./services.js"

async function readJson(request: IncomingMessage): Promise<unknown> {
  const chunks: Array<Buffer> = []
  for await (const chunk of request) {
    chunks.push(chunk as Buffer)
  }
  const text = Buffer.concat(chunks).toString("utf8")
  if (text === "") {
    return undefined
  }
  try {
    return JSON.parse(text)
  } catch {
    return text
  }
}

export async function startServer(
  layer: Layer.Layer<BookRepo | Ids | AuditLog>,
  port: number
): Promise<{ readonly url: string; readonly close: () => Promise<void> }> {
  const runtime = ManagedRuntime.make(layer)
  const server: Server = createServer(async (request, response) => {
    const result = await runtime.runPromise(
      handle({
        method: request.method ?? "GET",
        path: new URL(request.url ?? "/", "http://localhost").pathname,
        body: await readJson(request)
      })
    )
    response.writeHead(result.status, { "content-type": "application/json" })
    response.end(JSON.stringify(result.body, null, 2))
  })
  await new Promise<void>((resolve) => server.listen(port, resolve))
  const { port: actual } = server.address() as AddressInfo
  return {
    url: `http://localhost:${actual}`,
    close: async () => {
      await new Promise<void>((resolve) => server.close(() => resolve()))
      await runtime.dispose()
    }
  }
}
