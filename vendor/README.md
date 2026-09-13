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
