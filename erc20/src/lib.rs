pub mod contract;

mod error;
mod msg;
mod state;
#[cfg(test)]
mod tests;

pub use msg::{AllowanceResponse, BalanceResponse, HandleMsg, InitMsg, InitialBalance, QueryMsg};
pub use state::Constants;

#[cfg(target_arch = "wasm32")]
cosmwasm_std::create_entry_points!(contract);
