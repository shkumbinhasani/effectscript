# Library lending API in EffectScript

An HTTP API for a lending library: members borrow and return books, loans
have due dates, late returns cost a fee, a member can hold at most three
books, and data lives in SQLite. It is written in EffectScript (`.ef`), with
plain TypeScript where it talks to Node.

## Layers

```
src/
  domain/            what the library is; no Effect services, no I/O
    book.ef          Book, NewBook
    member.ef        Member, NewMember
    loan.ef          Loan
    policies.ef      loan period, loan limit, late fee rule
    errors.ef        BookNotFound, BookUnavailable, LoanLimitReached, ...
  application/       use cases; depend on ports, never on adapters
    ports.ef         BookRepository, MemberRepository, LoanRepository, Ids
    catalog.ef       addBook, listBooks, getBook
    members.ef       registerMember, getMember
    lending.ef       borrowBook, returnBook, memberLoans
  infrastructure/    adapters for the ports
    database.ef      SQLite connection as a scoped resource
    migrations.ef    schema versions (PRAGMA user_version)
    *-repository.ef  the repositories on SQLite
    ids.ef           random ids, and sequential ids for tests
    layers.ef        wires the repositories to the database
  http/              the edge: decode requests, call use cases, encode responses
    schemas.ef       request bodies as Effect Schema
    routes.ef        one effect function per route
    router.ef        route matching, error to status mapping, request logs
    responses.ef     status codes for every typed error
    presenters.ef    domain objects to JSON
    server.ts        Node http server running on a ManagedRuntime
  config.ef          PORT, DATABASE_PATH, LOG_LEVEL through Effect Config
  app.ef             the production layer
  main.ts            composition root: config, runtime, server, shutdown
test/
  support.ef         test layer: SQLite :memory:, sequential ids, TestClock
  policies.test.ef   the pure rules
  lending.test.ef    use cases as effect functions; time moved with TestClock
  http.test.ts       the routes end to end
```

Dependencies point inward: `http` and `infrastructure` depend on
`application`, which depends on `domain`. The use cases name what they need
in their `requires` clause (`requires BookRepository | LoanRepository | Ids`),
and only `app.ef` and the tests choose the adapters.

## Errors

- **Domain errors** (`BookUnavailable`, `LoanLimitReached`, ...) are typed
  failures. Each use case declares the ones it can produce in its `throws`
  clause, and the compiler checks the clause against the code.
- **Bad requests** fail with `InvalidRequest` at the edge, before a use case
  runs.
- **Database failures** are defects: a use case could not do anything about
  them. The router turns a defect into a logged 500.
- `router.ef` maps every typed error to a status code in one `switch`. The
  switch covers the whole union, so `handle` has the error type `never`:
  every request gets a response.

## Run

```sh
cargo build --release -p ts_goport --bin efc   # in the repository root
cd examples/library
npm ci
npm run build
npm test
npm start                                      # http://localhost:3000, library.db
```

```sh
curl -X POST localhost:3000/books -d '{"title": "SICP", "author": "Abelson & Sussman"}'
curl -X POST localhost:3000/members -d '{"name": "Ada", "email": "ada@example.com"}'
curl -X POST localhost:3000/books/<book id>/loans -d '{"memberId": "<member id>"}'
curl -X POST localhost:3000/books/<book id>/return
curl localhost:3000/members/<member id>/loans
```

| Route | Does |
|---|---|
| `GET /health` | liveness |
| `GET /books`, `GET /books/:id`, `POST /books` | catalog |
| `POST /members`, `GET /members/:id` | members |
| `POST /books/:id/loans` | borrow (`{"memberId": ...}`) |
| `POST /books/:id/return` | return; the response has the late fee |
| `GET /members/:id/loans` | loan history |
