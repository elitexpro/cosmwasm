# Migrating Contracts

This guide explains what is needed to upgrade contracts when migrating over major releases
of `cosmwasm`. Note that you can also view the [complete CHANGELOG](./CHANGELOG.md) to
understand the differences.

## 0.7.2 -> 0.8

`Cargo.toml` dependencies:

* Update to `schemars = "0.7"`
* Update to `snafu = "0.6.3"`
* Replace `cosmwasm = "0.7"` with `cosmwasm_std = "0.8"`
* Replace `cosmwasm_vm = "0.7"` with `cosmwasm_vm = "0.8"`
* Replace `cw_storage = "0.2"` with `cosmwasm_storage = "0.8"`

(Note: until release of `0.8`, you need to use git references for all `cosmwasm_*` packages)

`Cargo.toml` features:

* Replace `"cosmwasm/backtraces"` with `"cosmwasm-std/backtraces"`

Imports:

* Replace all `use cosmwasm::X::Y` with `use cosmwasm_std::Y`, except for mock
* Replace all `use cosmwasm::mock::Y` with `use cosmwasm_std::testing::Y`. This should only be used in test code.
* Replace `cw_storage:X` with `cosmwasm_storage::X`
* Replace `cosmwasm_std::Response` with `cosmwasm_std::HandleResponse` and `cosmwasm_std::InitResponse` (different type for each call)

`src/lib.rs`:

This has been re-written, but is generic boilerplate and should be (almost) the same in all contracts:

* copy the new version from [`contracts/queue`](https://github.com/CosmWasm/cosmwasm/blob/master/contracts/queue/src/lib.rs)
* Add `pub mod XYZ` directives for any modules you use besides `contract`

Contract Code:

* Add query to extern:
    * Before: `my_func<S: Storage, A: Api>(deps: &Extern<S, A>, ...`
    * After: `my_func<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, ...`
    * Remember to add `use cosmwasm_std::Querier;`
* `query` now returns `Result<Binary>` not `Result<Vec<u8>>`
    * You can also replace `to_vec(...)` with `to_binary(...)`
* No `.context(...)` is required after `from_slice` and `to_vec`, they return proper `cosmwasm_std::Error` variants on errors.
