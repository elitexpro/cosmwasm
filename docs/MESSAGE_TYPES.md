## CosmWasm message types

CosmWasm uses JSON for sending data from the host the Wasm contract and results
out of the Wasm contract. Such JSON messages are created in the client,
typically some JavaScript-based application. There the usage of JSON is feels
very natural for developers. However, JSON has signigicant limitations such as
the lack of a native binary type and inconsistent support for integers > 53 bit.
For this reason, the CosmWasm standard limrary `cosmwasm-std` shipts types that
ensure good user experience in JSON. The following table shows both stadard Rust
types as well as cosmwasm_std types and how they are encoded in JSON.

| Rust type           | JSON type[^1]                    | Example              | Note                                                                                                             |
| ------------------- | -------------------------------- | -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| bool                | `true` or `false`                | `true`               |                                                                                                                  |
| u32/i32             | number                           | `123`                |                                                                                                                  |
| u64/i64             | number                           | `123456`             | Supported in Rust and Go. Other implementations (`jq`, `JavaScript`) do not support the full uint64/int64 range. |
| usize/isize         | number                           | `123456`             | ⚠️ Discouraged as this type has a different size in unit tests (64 bit) and Wasm (32 bit)                        |
| String              | string                           | `"foo"`              |
| &str                | string                           | `"foo"`              | ⚠️ Unsuppored since message types must be owned (DeserializeOwned)                                               |
| Option\<T\>         | `null` or JSON type of `T`       | `null`, `{"foo":12}` |                                                                                                                  |
| Vec\<T\>            | array of JSON type of `T`        | `[1, 2, 3]`          |
| Vec\<u8\>           | array of numbers from 0 to 255   | `[187, 61, 11, 250]` | ⚠️ Discouraged as this encoding is not as compact as it can be. See `Binary`.                                    |
| struct MyType { … } | object                           | `{"foo":12}`         |                                                                                                                  |
| [Uint64]            | string containing number         | `"1234321"`          | Used to support full uint64 range in all implementations                                                         |
| [Uint128]           | string containing number         | `"1234321"`          |                                                                                                                  |
| [Uint256]           | string containing number         | `"1234321"`          |                                                                                                                  |
| [Uint512]           | string containing number         | `"1234321"`          |                                                                                                                  |
| [Decimal]           | string containing decimal number | `"55.6584"`          |                                                                                                                  |
| [Decimal256]        | string containing decimal number | `"55.6584"`          |                                                                                                                  |
| [Binary]            | string containing base64 data    | `"MTIzCg=="`         |                                                                                                                  |

[uint64]: https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Uint64.html
[uint128]: https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Uint128.html
[uint256]: https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Uint256.html
[uint512]: https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Uint512.html
[decimal]: https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Decimal.html
[decimal256]:
  https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Decimal256.html
[binary]: https://docs.rs/cosmwasm-std/1.1.1/cosmwasm_std/struct.Binary.html

[^1]: https://www.json.org/
