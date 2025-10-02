use anchor_lang::prelude::*;

#[event]
pub struct RedemptionRequested {
    pub user: Pubkey,
    pub amount: u64,
    pub vault_mint: Pubkey,
    pub mint: Pubkey,
}
