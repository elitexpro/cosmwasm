#!/bin/bash
set -o errexit -o nounset -o pipefail
command -v shellcheck > /dev/null && shellcheck "$0"

# Temporary incomplete testing command for development
(cd packages/vm \
  && cargo check --tests \
  && cargo check --features iterator --tests \
  && cargo check --features cranelift --tests \
  && cargo check --features cranelift,iterator --tests \
  && cargo test --features iterator \
  && cargo clippy --features iterator -- -D warnings)

# Contracts
for contract_dir in contracts/*/; do
  # 1. Build Wasm
  # 2. Run in Cranelift
  # 3. Run in Singlepass (fails on Windows)
  (cd "$contract_dir" && cargo wasm && cargo integration-test && cargo integration-test --no-default-features) || break;
done
