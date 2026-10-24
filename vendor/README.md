# Vendored Spin SDK

Pinned to `codeitlikemiley/spin-rust-sdk` rev
`a02d330fe9357be2d18e6deef400511195ce6f7f` (6.0.0).

`spin-sdk` `Connection::{open,query,execute}` call the blocking
`spin:postgres@4.2` imports (`open` / `query` / `execute`) instead of the
4.2 streaming `*-async` imports. Streaming query results add waitables that
trap on current Wasmtime when used from the HTTP handler:

`waitable cannot be used synchronously while added to a waitable set`

The public async methods stay the same; `QueryResult` is filled from the
complete row-set so `wasi-auth` and `ddd-cqrs-es` keep compiling.

`spin-sdk` `redis::Connection` binds the blocking `fermyon:spin/redis@2.0.0`
interface instead of `spin:redis@3.0.0`, whose functions are all
component-async and trap the same way (the tax-profile wake publish ran after
the Postgres commit and turned successful writes into HTTP 500).

# Vendored wit-bindgen

`wit-bindgen` 0.57.1 (the version `wasip3 0.7` pins) plus
bytecodealliance/wit-bindgen#1638, released upstream in 0.59.0:
`cancel_inter_task_stream_read` removes the inter-task wakeup stream from the
waitable set before the synchronous `stream.cancel-read`. Without it every
request traps after its response is sent. Remove this copy and the
`[patch.crates-io]` entry once `wasip3` depends on wit-bindgen >= 0.59.
