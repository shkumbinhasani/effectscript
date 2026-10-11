import { createServer, type IncomingMessage, type Server } from "node:http"
import type { AddressInfo } from "node:net"
import type { Effect, ManagedRuntime } from "effect"
import { handle, withRequestId } from "./router.js"

export type AppServices = Effect.Effect.Context<ReturnType<typeof handle>>

export type AppRuntime = ManagedRuntime.ManagedRuntime<AppServices, unknown>

async function readBody(request: IncomingMessage): Promise<unknown> {
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

export interface RunningServer {
  readonly url: string
  readonly close: () => Promise<void>
}

export async function serve(runtime: AppRuntime, port: number): Promise<RunningServer> {
  const server: Server = createServer(async (request, response) => {
    const result = await runtime.runPromise(
      withRequestId(
        handle({
          method: request.method ?? "GET",
          path: new URL(request.url ?? "/", "http://localhost").pathname,
          body: await readBody(request)
        })
      )
    )
    response.writeHead(result.status, { "content-type": "application/json" })
    response.end(JSON.stringify(result.body))
  })
  await new Promise<void>((resolve) => server.listen(port, resolve))
  const { port: actual } = server.address() as AddressInfo
  return {
    url: `http://localhost:${actual}`,
    close: () => new Promise<void>((resolve) => server.close(() => resolve()))
  }
}

