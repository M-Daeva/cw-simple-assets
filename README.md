# cw-simple-assets

Helpers to work with native and cw20 tokens in CosmWasm contracts

## Features



## Usage

```rust
use cosmwasm_std::{DepsMut, Env, MessageInfo, Response, StdResult, Uint128};
use cw_simple_assets::{check_funds, FundsType};

pub fn try_deposit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    sender: Option<String>,
    amount: Option<Uint128>,
) -> Result<Response, ContractError> {
    let (sender_address, asset_amount, asset_info) =
        check_funds(deps.as_ref(), &info, FundsType::Single { sender, amount })?;

    // ...
}
```

## Licenses

This repo is licensed under [Apache 2.0](./LICENSE).