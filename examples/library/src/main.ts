import { Effect, ManagedRuntime } from "effect"
import { AppLayer } from "./app.js"
import { AppConfig } from "./config.js"
import { serve } from "./http/server.js"

const config = await Effect.runPromise(AppConfig)
const runtime = ManagedRuntime.make(AppLayer(config))
const server = await serve(runtime, config.port)

console.log(`library API on ${server.url} (database ${config.databasePath})`)

async function shutdown() {
  await server.close()
  await runtime.dispose()
  process.exit(0)
}

process.once("SIGINT", shutdown)
process.once("SIGTERM", shutdown)
