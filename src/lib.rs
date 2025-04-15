use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    coin, coins, wasm_execute, Addr, Api, BankMsg, Coin, CosmosMsg, MessageInfo, StdError,
    StdResult, Uint128, WasmMsg,
};

use thiserror::Error;

#[cw_serde]
pub enum Token {
    Native { denom: String },
    Cw20 { address: Addr },
}

impl Token {
    pub fn new_native(denom: &str) -> Self {
        Self::Native {
            denom: denom.to_string(),
        }
    }

    pub fn new_cw20(address: &Addr) -> Self {
        Self::Cw20 {
            address: address.to_owned(),
        }
    }

    pub fn is_native(&self) -> bool {
        match self {
            Self::Native { denom: _ } => true,
            Self::Cw20 { address: _ } => false,
        }
    }

    pub fn try_get_native(&self) -> StdResult<String> {
        match self {
            Self::Native { denom } => Ok(denom.to_string()),
            Self::Cw20 { address: _ } => Err(AssetError::AssetIsNotFound)?,
        }
    }

    pub fn try_get_cw20(&self) -> StdResult<Addr> {
        match self {
            Self::Native { denom: _ } => Err(AssetError::AssetIsNotFound)?,
            Self::Cw20 { address } => Ok(address.to_owned()),
        }
    }

    pub fn get_symbol(&self) -> String {
        match self {
            Self::Native { denom } => denom.to_string(),
            Self::Cw20 { address } => address.to_string(),
        }
    }
}

impl From<String> for Token {
    fn from(denom: String) -> Self {
        Self::Native { denom }
    }
}

impl From<Addr> for Token {
    fn from(address: Addr) -> Self {
        Self::Cw20 { address }
    }
}

#[cw_serde]
pub enum TokenUnverified {
    Native { denom: String },
    Cw20 { address: String },
}

impl TokenUnverified {
    pub fn new_native(denom: &str) -> Self {
        Self::Native {
            denom: denom.to_string(),
        }
    }

    pub fn new_cw20(address: &str) -> Self {
        Self::Cw20 {
            address: address.to_string(),
        }
    }

    pub fn verify(&self, api: &dyn Api) -> StdResult<Token> {
        match self {
            Self::Cw20 { address } => Ok(Token::new_cw20(&api.addr_validate(address)?)),
            Self::Native { denom } => Ok(Token::new_native(denom)),
        }
    }

    pub fn get_symbol(&self) -> String {
        match self.to_owned() {
            Self::Native { denom } => denom,
            Self::Cw20 { address } => address,
        }
    }
}

impl From<Token> for TokenUnverified {
    fn from(token: Token) -> Self {
        match token {
            Token::Native { denom } => Self::Native { denom },
            Token::Cw20 { address } => Self::Cw20 {
                address: address.to_string(),
            },
        }
    }
}

#[cw_serde]
pub struct Currency<T: From<Token>> {
    pub token: T,
    pub decimals: u8,
}

impl Default for Currency<Token> {
    fn default() -> Self {
        Self::new(&Token::new_native(&String::default()), 0)
    }
}

impl<T: From<Token> + Clone> Currency<T> {
    pub fn new(denom_or_address: &T, decimals: u8) -> Self {
        Self {
            token: denom_or_address.to_owned(),
            decimals,
        }
    }
}

#[cw_serde]
pub struct InfoResp {
    pub sender: Addr,
    pub asset_amount: Uint128,
    pub asset_token: Token,
}

#[cw_serde]
pub enum Funds {
    Empty,
    Single {
        sender: Option<String>,
        amount: Option<Uint128>,
    },
}

impl Funds {
    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn single(sender: Option<String>, amount: Option<Uint128>) -> Self {
        Self::Single { sender, amount }
    }

    /// Supports both native and cw20 tokens                                        \
    /// * Funds::empty() to check if info.funds is empty                            \
    /// * Funds::single(None, None) to check native token                           \
    /// * Funds::single(Some(msg.sender), Some(msg.amount)) to check cw20 token
    pub fn check(&self, api: &dyn Api, info: &MessageInfo) -> StdResult<InfoResp> {
        match self {
            Funds::Empty => {
                nonpayable(info)?;

                Ok(InfoResp {
                    sender: info.sender.to_owned(),
                    asset_amount: Uint128::zero(),
                    asset_token: Token::new_native(&String::default()),
                })
            }
            Funds::Single { sender, amount } => {
                if sender.as_ref().is_none() || amount.is_none() {
                    let Coin { denom, amount } = one_coin(info)?;

                    Ok(InfoResp {
                        sender: info.sender.to_owned(),
                        asset_amount: amount,
                        asset_token: Token::new_native(&denom),
                    })
                } else {
                    Ok(InfoResp {
                        sender: api.addr_validate(
                            sender.as_ref().ok_or(AssetError::WrongFundsCombination)?,
                        )?,
                        asset_amount: amount.ok_or(AssetError::WrongFundsCombination)?,
                        asset_token: Token::new_cw20(&info.sender),
                    })
                }
            }
        }
    }
}

pub fn add_funds_to_exec_msg(
    exec_msg: &WasmMsg,
    funds_list: &[(Uint128, Token)],
) -> StdResult<WasmMsg> {
    let mut native_tokens: Vec<Coin> = vec![];
    let mut cw20_tokens: Vec<(Uint128, Addr)> = vec![];

    for (amount, token) in funds_list {
        match token {
            Token::Native { denom } => {
                native_tokens.push(coin(amount.u128(), denom));
            }
            Token::Cw20 { address } => {
                cw20_tokens.push((*amount, address.to_owned()));
            }
        }
    }

    match exec_msg {
        WasmMsg::Execute {
            contract_addr, msg, ..
        } => {
            // Case 1 `Deposit` - only native tokens
            if cw20_tokens.is_empty() {
                return Ok(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: msg.to_owned(),
                    funds: native_tokens,
                });
            }

            // Case 2 `Swap` - only single cw20 token
            if (cw20_tokens.len() == 1) && native_tokens.is_empty() {
                let (amount, token_address) =
                    cw20_tokens.first().ok_or(AssetError::AssetIsNotFound)?;

                return wasm_execute(
                    token_address,
                    &cw20::Cw20ExecuteMsg::Send {
                        contract: contract_addr.to_string(),
                        amount: amount.to_owned(),
                        msg: msg.to_owned(),
                    },
                    vec![],
                );
            }

            Err(AssetError::WrongFundsCombination)?
        }
        _ => Err(AssetError::WrongActionType)?,
    }
}

pub fn get_transfer_msg(recipient: &Addr, amount: Uint128, token: &Token) -> StdResult<CosmosMsg> {
    Ok(match token {
        Token::Native { denom } => CosmosMsg::Bank(BankMsg::Send {
            to_address: recipient.to_string(),
            amount: coins(amount.u128(), denom),
        }),
        Token::Cw20 { address } => CosmosMsg::Wasm(wasm_execute(
            address,
            &cw20::Cw20ExecuteMsg::Transfer {
                recipient: recipient.to_string(),
                amount,
            },
            vec![],
        )?),
    })
}

/// If exactly one coin was sent, returns it regardless of denom.
/// Returns error if 0 or 2+ coins were sent
fn one_coin(info: &MessageInfo) -> StdResult<Coin> {
    if info.funds.len() != 1 {
        Err(AssetError::NonSingleDenom)?;
    }

    if let Some(coin) = info.funds.first() {
        if !coin.amount.is_zero() {
            return Ok(coin.to_owned());
        }
    }

    Err(AssetError::ZeroCoins)?
}

/// returns an error if any coins were sent
fn nonpayable(info: &MessageInfo) -> StdResult<()> {
    if !info.funds.is_empty() {
        Err(AssetError::ShouldNotAcceptFunds)?;
    }

    Ok(())
}

#[derive(Error, Debug, PartialEq)]
pub enum AssetError {
    #[error("Asset isn't found!")]
    AssetIsNotFound,

    #[error("Wrong funds combination!")]
    WrongFundsCombination,

    #[error("Wrong action type!")]
    WrongActionType,

    #[error("Coins amount is zero!")]
    ZeroCoins,

    #[error("Amount of denoms isn't equal 1!")]
    NonSingleDenom,

    #[error("This message doesn't accept funds!")]
    ShouldNotAcceptFunds,
}

impl From<AssetError> for StdError {
    fn from(asset_error: AssetError) -> Self {
        Self::generic_err(asset_error.to_string())
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use cosmwasm_std::testing::{message_info, mock_dependencies};

    #[test]
    fn test_single_coin() -> StdResult<()> {
        const ADMIN: &str = "cosmwasm105yqjjdgl00nzwyj9aua98zgetdn4qyhukjf5t";
        const AMOUNT: u128 = 100;
        const DENOM: &str = "cosm";

        let deps = mock_dependencies();
        let info = message_info(&Addr::unchecked(ADMIN), &coins(AMOUNT, DENOM));
        let info_resp = Funds::single(None, None).check(&deps.api, &info)?;

        assert_eq!(
            info_resp,
            InfoResp {
                sender: Addr::unchecked(ADMIN),
                asset_amount: Uint128::new(AMOUNT),
                asset_token: Token::new_native(DENOM),
            }
        );

        Ok(())
    }
}
