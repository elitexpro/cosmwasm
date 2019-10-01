# CosmWasm

**Web Assembly Smart Contracts for the Cosmos SDK**

This repo provides a useful functionality to build smart contracts that
are compatible with Cosmos SDK runtime, [currently being developed](https://github.com/cosmwasm/cosmos-sdk/issues).

## Creating a Smart Contract

You can see some examples of contracts under the `contracts` directory.
We aim to provide more tooling to help this process, but for now it is a manual step.
You can do this in the `contracts` directory if you are working in this project, or
wherever you want in your own project. 

You can follow more instructions on how to [configure a library for wasm](./Building.md)

## API entry points

Web Assembly contracts are basically black boxes. The have no default entry points,
and no access to the outside world by default. To make them useful, we need to add
a few elements. 

We explain [how to create entry points](./EntryPoints.md) in general for
rust-wasm tooling, as well as [document the required API for CosmWasm contracts](./API.md)



## Compiling the Smart Contract

## Testing the Smart Contract

