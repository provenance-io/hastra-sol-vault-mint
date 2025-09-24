pub mod account_structs;
/// # Sol Vault Mint - Token Staking System
///
/// ## Business Process Flow
///
/// 1. Initial Setup:
///    - Admin creates two token types: Vault (USDC), Mint (wYLDS)
///    - Admin initializes program with token addresses
///    - Admin configures vault token account to hold deposited tokens
///
/// 2. User Staking Flow:
///    a. Deposit Phase:
///       - User deposits vault tokens (USDC)
///       - System securely stores tokens in vault account
///       - User receives equivalent mint tokens (wYLDS)
///
/// 3. Withdrawal Flow:
///    a. Redemption:
///       - Original vault tokens (USDC) returned to user, burning mint tokens (wYLDS)
///
/// 4. Administrative Functions:
///    - Update token configurations if needed
///    - Manage mint authorities
///    - Monitor vault token accounts
///
/// Security is maintained through PDAs (Program Derived Addresses) and strict
/// token authority controls. All token operations are atomic and validated
/// through Solana's transaction model.
pub mod error;
mod guard;
pub mod processor;
pub mod state;

use account_structs::*;
use anchor_lang::prelude::*;

declare_id!("FhqGQTSwE9JZqWevkMHw3SHAt9st4vfDdbZZsyXcoKm6");

#[program]
pub mod hastra_sol_vault_mint {
    use super::*;

    /// Initializes the vault program with the required token configurations:
    /// - vault_mint: The token that users deposit (e.g., USDC)
    /// - mint: The token users receive when deposit received (e.g., wYLDS)
    pub fn initialize(
        ctx: Context<Initialize>,
        vault_mint: Pubkey,
        mint: Pubkey,
        freeze_administrators: Vec<Pubkey>,
        rewards_administrators: Vec<Pubkey>,
    ) -> Result<()> {
        processor::initialize(
            ctx,
            vault_mint,
            mint,
            freeze_administrators,
            rewards_administrators,
        )
    }

    /// Handles user deposits of vault tokens (e.g., USDC):
    /// - Transfers vault tokens to program vault account
    /// - Mints equivalent amount of mint tokens (e.g., wYLDS) to user
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        processor::deposit(ctx, amount)
    }

    /// The redeem function allows users to withdraw their original vault tokens:
    /// - Transfers vault tokens from a program vault account to user
    /// - Burns the corresponding amount of mint tokens (e.g., wYLDS) from user
    pub fn redeem(ctx: Context<Redeem>, amount: u64) -> Result<()> {
        processor::redeem(ctx, amount)
    }

    /// Sets the mint authority for a specified token type
    /// Used to configure program control over token minting
    pub fn set_mint_authority(ctx: Context<SetMintAuthority>, new_authority: Pubkey) -> Result<()> {
        processor::set_mint_authority(ctx, new_authority)
    }
    
    pub fn update_freeze_administrators(
        ctx: Context<UpdateFreezeAdministrators>,
        new_administrators: Vec<Pubkey>,
    ) -> Result<()> {
        processor::update_freeze_administrators(ctx, new_administrators)
    }

    pub fn freeze_token_account(ctx: Context<FreezeTokenAccount>) -> Result<()> {
        processor::freeze_token_account(ctx)
    }
    pub fn thaw_token_account(ctx: Context<ThawTokenAccount>) -> Result<()> {
        processor::thaw_token_account(ctx)
    }

    pub fn update_rewards_administrators(
        ctx: Context<UpdateRewardsAdministrators>,
        new_administrators: Vec<Pubkey>,
    ) -> Result<()> {
        processor::update_rewards_administrators(ctx, new_administrators)
    }

    pub fn create_rewards_epoch(
        ctx: Context<CreateRewardsEpoch>,
        index: u64,
        merkle_root: [u8; 32],
        total: u64,
    ) -> Result<()> {
        processor::create_rewards_epoch(ctx, index, merkle_root, total)
    }

    /// This is the classic “airdrop/claim per epoch” design
    /// High-level idea:
    /// 	1.	Off-chain (admin does this each epoch):
    /// 	•	Calculate each user’s reward for this epoch.
    /// 	•	Build a Merkle tree of (user, amount, epoch_index).
    /// 	•	Publish the Merkle root on-chain with the create_rewards_epoch function above.
    ///
    /// 	2.	On-chain:
    /// 	•	Store each epoch’s Merkle root in a PDA.
    /// 	•	When a user claims, they present (amount, proof) for their pubkey.
    /// 	•	The program verifies the Merkle proof against the root.
    /// 	•	If valid, transfer reward tokens (sYLDS) from the rewards vault to the user's staking mint token account.
    /// 	•	Mark the claim as redeemed so they can’t double-claim.
    pub fn claim_rewards(ctx: Context<ClaimRewards>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
        processor::claim_rewards(ctx, amount, proof)
    }
}
    
