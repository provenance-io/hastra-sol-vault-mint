use anchor_lang::prelude::*;

#[event]
pub struct DepositEvent {
    pub user: Pubkey,
    pub amount: u64,
    pub mint: Pubkey,
    pub vault: Pubkey,
}

#[event]
pub struct RewardsClaimed {
    pub user: Pubkey,
    pub epoch: u64,
    pub amount: u64,
    pub mint: Pubkey,
    pub vault: Pubkey,
}

#[event]
pub struct RedemptionRequested {
    pub user: Pubkey,
    pub amount: u64,
    pub vault_mint: Pubkey,
    pub mint: Pubkey,
}

#[event]
pub struct RedeemCompleted {
    pub user: Pubkey,
    pub admin: Pubkey,
    pub amount: u64,
    pub mint: Pubkey,
    pub vault: Pubkey,
}

