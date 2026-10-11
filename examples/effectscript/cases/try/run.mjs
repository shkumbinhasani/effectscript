import { Effect, Exit, Fiber } from "effect"
import { cleanup, findOrGuest, firstFound, inferred, log, nested } from "./out/try.js"

const results = [
  await Effect.runPromise(findOrGuest("1")),
  await Effect.runPromise(findOrGuest("7")),
  (await Effect.runPromiseExit(findOrGuest("db")))._tag,
  await Effect.runPromise(firstFound(["9", "db", "1"])),
  await Effect.runPromise(firstFound(["9"])),
  await Effect.runPromise(nested("1")),
  await Effect.runPromise(nested("7")),
  await Effect.runPromise(nested("db")),
  (await Effect.runPromise(inferred("7"))).name
]
const expected = ["Ada", "guest 7", "Failure", "Ada", "nobody", 1, 2, 4, "NotFound"]
if (JSON.stringify(results) !== JSON.stringify(expected)) {
  throw new Error(`FAILED: ${JSON.stringify(results)}`)
}
const exit = await Effect.runPromise(
  Effect.gen(function* () {
    const fiber = yield* Effect.fork(cleanup())
    yield* Effect.sleep(10)
    return yield* Fiber.interrupt(fiber)
  })
)
if (!Exit.isInterrupted(exit) || log.join(",") !== "done 1,done 7,done db,interrupted cleanup") {
  throw new Error(`FAILED: ${log.join(",")}`)
}
console.log("ok - try, catch and finally")
