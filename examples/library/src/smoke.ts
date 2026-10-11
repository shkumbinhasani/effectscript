import { Effect, Exit } from "effect"
import { borrowBook, returnBook } from "./library.js"
import { startServer } from "./server.js"
import { makeAppLayer, seedBooks } from "./services.js"

let failures = 0
function check(label: string, ok: boolean, detail?: unknown) {
  console.log(`${ok ? "ok" : "FAILED"} - ${label}`)
  if (!ok) {
    failures++
    console.log("  ", JSON.stringify(detail))
  }
}

const layer = makeAppLayer({ seed: seedBooks, echo: false })

const lazy = borrowBook("b1", { member: "ada" })
check("calling an effect function runs nothing", Effect.isEffect(lazy))

const typed = await Effect.runPromiseExit(Effect.provide(returnBook("b1"), layer))
check(
  "returning a book that is not borrowed fails with NotBorrowed",
  Exit.isFailure(typed) && typed.cause._tag === "Fail" && typed.cause.error._tag === "NotBorrowed",
  typed
)

const { url, close } = await startServer(layer, 0)
async function call(method: string, path: string, body?: unknown) {
  const response = await fetch(url + path, {
    method,
    body: body === undefined ? undefined : JSON.stringify(body)
  })
  return { status: response.status, body: (await response.json()) as any }
}

try {
  const list = await call("GET", "/books")
  check("GET /books lists the seed books", list.status === 200 && list.body.length === 2, list)

  const added = await call("POST", "/books", { title: "Effect in Practice", author: "Grace" })
  check("POST /books adds a book", added.status === 201 && added.body.id === "b1", added)

  const invalid = await call("POST", "/books", { title: "  " })
  check("POST /books validates the body", invalid.status === 400 && /author/.test(invalid.body.error), invalid)

  const borrowed = await call("POST", "/books/b2/borrow", { member: "ada" })
  check("borrowing sets a loan", borrowed.status === 200 && borrowed.body.loan.member === "ada", borrowed)

  const twice = await call("POST", "/books/b2/borrow", { member: "linus" })
  check("borrowing twice is a conflict", twice.status === 409 && /ada/.test(twice.body.error), twice)

  const missing = await call("POST", "/books/zz/borrow", { member: "ada" })
  check("an unknown book is 404", missing.status === 404, missing)

  const returned = await call("POST", "/books/b2/return")
  check("returning on time costs nothing", returned.status === 200 && returned.body.fee === 0, returned)

  const notBorrowed = await call("POST", "/books/b2/return")
  check("returning twice is a conflict", notBorrowed.status === 409, notBorrowed)

  const route = await call("DELETE", "/books")
  check("an unknown route is 404", route.status === 404, route)

  const audit = await call("GET", "/audit")
  check(
    "the audit log records every change",
    JSON.stringify(audit.body) ===
      JSON.stringify(['added b1 "Effect in Practice"', "ada borrowed b2", "ada returned b2"]),
    audit
  )
} finally {
  await close()
}

if (failures > 0) {
  process.exitCode = 1
}
