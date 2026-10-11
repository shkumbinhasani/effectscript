import { strict as assert } from "node:assert"
import { after, before, test } from "node:test"
import { ManagedRuntime } from "effect"
import { serve, type RunningServer } from "../src/http/server.js"
import { TestLayer } from "./support.js"

const runtime = ManagedRuntime.make(TestLayer)
let server: RunningServer

before(async () => {
  server = await serve(runtime, 0)
})

after(async () => {
  await server.close()
  await runtime.dispose()
})

async function call(method: string, path: string, body?: unknown) {
  const response = await fetch(server.url + path, {
    method,
    body: body === undefined ? undefined : JSON.stringify(body)
  })
  return { status: response.status, body: (await response.json()) as any }
}

test("the full lending flow over HTTP", async () => {
  assert.deepEqual(await call("GET", "/health"), { status: 200, body: { status: "ok" } })

  const book = await call("POST", "/books", { title: "SICP", author: "Abelson & Sussman" })
  assert.equal(book.status, 201)

  const member = await call("POST", "/members", { name: "Ada", email: "Ada@Example.com" })
  assert.equal(member.status, 201)
  assert.equal(member.body.email, "ada@example.com")

  const loan = await call("POST", `/books/${book.body.id}/loans`, { memberId: member.body.id })
  assert.equal(loan.status, 201)
  assert.equal(loan.body.returnedAt, null)

  const again = await call("POST", `/books/${book.body.id}/loans`, { memberId: member.body.id })
  assert.equal(again.status, 409)
  assert.equal(again.body.error.code, "book_unavailable")

  const returned = await call("POST", `/books/${book.body.id}/return`)
  assert.equal(returned.status, 200)
  assert.equal(returned.body.feeInCents, 0)

  const loans = await call("GET", `/members/${member.body.id}/loans`)
  assert.equal(loans.body.length, 1)
})

test("errors map to status codes", async () => {
  const invalid = await call("POST", "/books", { title: "", author: "x", extra: 1 })
  assert.equal(invalid.status, 400)
  assert.equal(invalid.body.error.code, "invalid_request")

  const badEmail = await call("POST", "/members", { name: "Bob", email: "nope" })
  assert.equal(badEmail.status, 400)

  await call("POST", "/members", { name: "Cy", email: "cy@example.com" })
  const taken = await call("POST", "/members", { name: "Cy 2", email: "CY@example.com" })
  assert.equal(taken.body.error.code, "email_taken")

  assert.equal((await call("GET", "/books/nope")).body.error.code, "book_not_found")
  assert.equal((await call("GET", "/members/nope/loans")).body.error.code, "member_not_found")
  assert.equal((await call("POST", "/books/nope/return")).body.error.code, "book_not_found")
  assert.equal((await call("DELETE", "/books")).body.error.code, "route_not_found")
})
