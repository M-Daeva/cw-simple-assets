# cw-simple-assets

Helpers to work with native and cw20 tokens in CosmWasm contracts

## Features

- Types for native and cw20 tokens
- Validation for native and cw20 tokens sent to a contract

## Usage

```rust
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult, Uint128};
use cw_simple_assets::{Funds, InfoResp};

pub fn try_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Option<String>,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let InfoResp {
            sender,
            asset_amount,
            asset_token,
        } = Funds::single(sender, amount).check(&deps.api, &info)?;

    // ...
}
```

## Licenses

This repo is licensed under [Apache 2.0](./LICENSE).