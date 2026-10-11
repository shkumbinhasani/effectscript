# Library lending API in EffectScript

A small HTTP API for lending books, written in EffectScript. Members borrow
and return books, loans have due dates, late returns cost a fee, request
bodies are validated, and every change goes to an audit log.

| File | Language | What it holds |
|---|---|---|
| `src/domain.ef` | EffectScript | Book and loan types, request schemas, typed errors, the late fee rule |
| `src/services.ef` | EffectScript | The `BookRepo`, `Ids` and `AuditLog` services and their in-memory layers |
| `src/library.ef` | EffectScript | The use cases: list, add, borrow, return, audit trail |
| `src/http.ef` | EffectScript | Routing and the mapping from typed errors to HTTP status codes |
| `src/server.ts` | TypeScript | The Node HTTP server, which runs `handle` on a `ManagedRuntime` |
| `src/main.ts` | TypeScript | Starts the server |
| `src/smoke.ts` | TypeScript | Starts the server on a free port and checks every route |

```sh
cargo build --release -p ts_goport --bin efc   # in the repository root
cd examples/library
npm ci
npm run build
npm test         # 12 checks
npm start        # http://localhost:3000
```

```sh
curl localhost:3000/books
curl -X POST localhost:3000/books -d '{"title": "Effect in Practice", "author": "Grace"}'
curl -X POST localhost:3000/books/b2/borrow -d '{"member": "ada"}'
curl -X POST localhost:3000/books/b2/return
curl localhost:3000/audit
```
