# Example contracts

Those contracts are made for development purpose only. For more realistic
example contracts, see
[cosmwasm-examples](https://github.com/CosmWasm/cosmwasm-examples).

## Optimized builds

Those development contracts are used for testing in other repos, e.g. in
[go-cosmwasm](https://github.com/CosmWasm/go-cosmwasm/tree/master/api/testdata)
or
[cosmjs](https://github.com/CosmWasm/cosmjs/tree/master/scripts/wasmd/contracts).

They are [built and deployed](https://github.com/CosmWasm/cosmwasm/releases) by
the CI for every release tag. In case you need to build them manually for some
reason, use the following commands:

```sh
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="devcontract_cache_burner",target=/code/contracts/burner/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.10.5 ./contracts/burner

docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="devcontract_cache_hackatom",target=/code/contracts/hackatom/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.10.5 ./contracts/hackatom

docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="devcontract_cache_queue",target=/code/contracts/queue/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.10.5 ./contracts/queue

docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="devcontract_cache_reflect",target=/code/contracts/reflect/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.10.5 ./contracts/reflect

docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="devcontract_cache_staking",target=/code/contracts/staking/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.10.5 ./contracts/staking
```

## Entry points

The development contracts in this folder contain a variety of different entry
points in order to demonstrate and test the flexibility we have.

| Contract | Macro                                         | Has `query` | Has `migrate` |
| -------- | --------------------------------------------- | ----------- | ------------- |
| burner   | `#[entry_point]`                              | yes         | yes           |
| hackatom | [`create_entry_points_with_migration!`][cepm] | yes         | yes           |
| queue    | mixed<sup>1</sup>                             | yes         | yes           |
| reflect  | [`create_entry_points!`][cep]                 | yes         | no            |
| staking  | `#[entry_point]`                              | yes         | no            |

<sup>1</sup> Because we can. Don't try this at home.

[cepm]:
  https://docs.rs/cosmwasm-std/0.13.0/cosmwasm_std/macro.create_entry_points_with_migration.html
[cep]:
  https://docs.rs/cosmwasm-std/0.13.0/cosmwasm_std/macro.create_entry_points.html
