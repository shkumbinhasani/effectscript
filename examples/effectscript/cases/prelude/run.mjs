import { Effect } from "effect"
import { main, read, ConfigLive, shadowed } from "./out/prelude.js"

const name = await Effect.runPromise(main)
const missing = await Effect.runPromise(
  read("other").pipe(Effect.either, Effect.provide(ConfigLive))
)
if (name !== "effectscript" || missing._tag !== "Left" || missing.left._tag !== "Missing" || shadowed(1) !== 2) {
  throw new Error("FAILED: prelude")
}
console.log("ok - prelude names need no import")
