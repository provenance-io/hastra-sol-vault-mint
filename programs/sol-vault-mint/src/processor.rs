use crate::account_structs::*;
use crate::error::*;
use crate::events::*;
use crate::guard::validate_program_update_authority;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Burn, MintTo, Transfer};

pub fn initialize(
    ctx: Context<Initialize>,
    vault_mint: Pubkey,
    mint: Pubkey,
    freeze_administrators: Vec<Pubkey>,
    rewards_administrators: Vec<Pubkey>,
) -> Result<()> {

    msg!("Initializing with vault_mint: {}", vault_mint);
    msg!("Vault mint account: {}", ctx.accounts.vault_mint.key());

    require!(
        freeze_administrators.len() <= 5,
        CustomErrorCode::TooManyAdministrators
    );

    let config = &mut ctx.accounts.config;
    config.vault = vault_mint;
    config.mint = mint;
    config.freeze_administrators = freeze_administrators;
    config.rewards_administrators = rewards_administrators;
    config.vault_authority = ctx.accounts.vault_token_account.owner;
    config.bump = ctx.bumps.config;

    // The redeem vault token account must be owned by the program-derived address (PDA)
    // and is a token account that holds the deposited vault tokens (e.g., USDC).
    // This ensures that only the program can move tokens out of this account.
    // Only set vault token account to PDA authority if it's not already set to vault_authority

    if ctx.accounts.redeem_vault_token_account.owner == ctx.accounts.signer.key() {
       let seeds: &[&[u8]] = &[b"redeem_vault_authority", &[ctx.bumps.redeem_vault_authority]];
       let signer = &[&seeds[..]];
       token::set_authority(
           CpiContext::new_with_signer(
               ctx.accounts.token_program.to_account_info(),
               token::SetAuthority {
                   account_or_mint: ctx.accounts.redeem_vault_token_account.to_account_info(),
                   current_authority: ctx.accounts.signer.to_account_info(),
               },
               signer,
           ),
           AuthorityType::AccountOwner,
           Some(ctx.accounts.redeem_vault_authority.key()),
       )?;
    }

    Ok(())
}

pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, CustomErrorCode::InvalidAmount);

    // Validate that vault_token_account is owned by the configured vault authority
    require!(
        ctx.accounts.vault_token_account.owner == ctx.accounts.config.vault_authority,
        CustomErrorCode::InvalidVaultAuthority
    );

    let cpi_accounts = Transfer {
        from: ctx.accounts.user_vault_token_account.to_account_info(),
        to: ctx.accounts.vault_token_account.to_account_info(),
        authority: ctx.accounts.signer.to_account_info(),
    };
    token::transfer(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts),
        amount,
    )?;

    let seeds: &[&[u8]] = &[b"mint_authority", &[ctx.bumps.mint_authority]];
    let signer = &[&seeds[..]];
    let cpi_accounts = MintTo {
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.user_mint_token_account.to_account_info(),
        authority: ctx.accounts.mint_authority.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        ),
        amount,
    )?;
    Ok(())
}

pub fn request_redeem(ctx: Context<RequestRedeem>, amount: u64) -> Result<()> {
    require!(amount > 0, CustomErrorCode::InvalidAmount);

    // Check user's mint token balance
    let user_balance = ctx.accounts.user_mint_token_account.amount;
    require!(user_balance >= amount, CustomErrorCode::InsufficientBalance);

    emit!(RedemptionRequested {
        user: ctx.accounts.signer.key(),
        amount,
        vault_mint: ctx.accounts.config.vault,
        mint: ctx.accounts.config.mint,
    });

    // Record the request (creates a lock on the user)
    let request = &mut ctx.accounts.redemption_request;
    request.user = ctx.accounts.signer.key();
    request.amount = amount;
    request.vault_mint = ctx.accounts.config.vault;
    request.mint = ctx.accounts.config.mint;
    request.bump = ctx.bumps.redemption_request;

    Ok(())
}

pub fn complete_redeem(ctx: Context<CompleteRedeem>) -> Result<()> {
    // Only rewards administrators can call this
    require!(
        ctx.accounts.config.rewards_administrators.contains(&ctx.accounts.admin.key()),
        CustomErrorCode::InvalidRewardsAdministrator
    );

    let request = &mut ctx.accounts.redemption_request;

    // Use minimum of user's current balance or original requested amount
    let user_mint_balance = ctx.accounts.user_mint_token_account.amount;
    let amount_to_redeem = std::cmp::min(user_mint_balance, request.amount);
    require!(amount_to_redeem > 0, CustomErrorCode::InvalidAmount);

    let vault_balance = ctx.accounts.redeem_vault_token_account.amount;
    require!(vault_balance >= amount_to_redeem, CustomErrorCode::InsufficientVaultBalance);

    // Burn mint tokens from user
    let burn_accounts = Burn {
        mint: ctx.accounts.mint.to_account_info(),
        from: ctx.accounts.user_mint_token_account.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };
    token::burn(
        CpiContext::new(ctx.accounts.token_program.to_account_info(), burn_accounts),
        amount_to_redeem,
    )?;

    // Transfer vault mint (e.g. USDC) from redemption vault to user
    let seeds: &[&[u8]] = &[b"redeem_vault_authority", &[ctx.bumps.redeem_vault_authority]];
    let signer = &[&seeds[..]];
    let transfer_accounts = Transfer {
        from: ctx.accounts.redeem_vault_token_account.to_account_info(),
        to: ctx.accounts.user_vault_token_account.to_account_info(),
        authority: ctx.accounts.redeem_vault_authority.to_account_info(),
    };
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_accounts,
            signer,
        ),
        amount_to_redeem,
    )?;

    Ok(())
}

// Set the mint token's freeze authority to the program PDA
// Update the list of freeze administrators (only program update authority can do this)
pub fn update_freeze_administrators(
    ctx: Context<UpdateFreezeAdministrators>,
    new_administrators: Vec<Pubkey>,
) -> Result<()> {
    // Validate that the signer is the program's update authority
    validate_program_update_authority(&ctx.accounts.program_data, &ctx.accounts.signer)?;

    let config = &mut ctx.accounts.config;

    require!(
        new_administrators.len() <= 5,
        CustomErrorCode::TooManyAdministrators
    );

    config.freeze_administrators = new_administrators;

    msg!(
        "Freeze administrators updated. New count: {}",
        config.freeze_administrators.len()
    );
    Ok(())
}

// Set the mint token's rewards authority to the program PDA
// Update the list of rewards administrators (only program update authority can do this)
pub fn update_rewards_administrators(
    ctx: Context<UpdateRewardsAdministrators>,
    new_administrators: Vec<Pubkey>,
) -> Result<()> {
    // Validate that the signer is the program's update authority
    validate_program_update_authority(&ctx.accounts.program_data, &ctx.accounts.signer)?;

    let config = &mut ctx.accounts.config;

    require!(
        new_administrators.len() <= 5,
        CustomErrorCode::TooManyAdministrators
    );

    config.rewards_administrators = new_administrators;

    msg!(
        "Rewards administrators updated. New count: {}",
        config.freeze_administrators.len()
    );
    Ok(())
}

// Freeze a specific token account (only freeze administrators can do this)
pub fn freeze_token_account(ctx: Context<FreezeTokenAccount>) -> Result<()> {
    let config = &ctx.accounts.config;
    let signer = ctx.accounts.signer.key();

    // Verify signer is a freeze administrator
    require!(
        config.freeze_administrators.contains(&signer),
        CustomErrorCode::UnauthorizedFreezeAdministrator
    );

    let freeze_authority_seeds: &[&[&[u8]]] =
        &[&[b"freeze_authority", &[ctx.bumps.freeze_authority_pda]]];

    let cpi_accounts = token::FreezeAccount {
        account: ctx.accounts.token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        authority: ctx.accounts.freeze_authority_pda.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        freeze_authority_seeds,
    );

    token::freeze_account(cpi_ctx)?;

    msg!(
        "Token account {} frozen by administrator {}",
        ctx.accounts.token_account.key(),
        signer
    );
    Ok(())
}

// Thaw a specific token account (only freeze administrators can do this)
pub fn thaw_token_account(ctx: Context<ThawTokenAccount>) -> Result<()> {
    let config = &ctx.accounts.config;
    let signer = ctx.accounts.signer.key();

    // Verify signer is a freeze administrator
    require!(
        config.freeze_administrators.contains(&signer),
        CustomErrorCode::UnauthorizedFreezeAdministrator
    );

    let freeze_authority_seeds: &[&[&[u8]]] =
        &[&[b"freeze_authority", &[ctx.bumps.freeze_authority_pda]]];

    let cpi_accounts = token::ThawAccount {
        account: ctx.accounts.token_account.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        authority: ctx.accounts.freeze_authority_pda.to_account_info(),
    };

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        freeze_authority_seeds,
    );

    token::thaw_account(cpi_ctx)?;

    msg!(
        "Token account {} thawed by administrator {}",
        ctx.accounts.token_account.key(),
        signer
    );
    Ok(())
}

pub fn create_rewards_epoch(
    ctx: Context<CreateRewardsEpoch>,
    index: u64,
    merkle_root: [u8; 32],
    total: u64,
) -> Result<()> {
    require!(
        ctx.accounts
            .config
            .rewards_administrators
            .contains(&ctx.accounts.admin.key()),
        CustomErrorCode::InvalidRewardsAdministrator
    );
    let e = &mut ctx.accounts.epoch;
    e.index = index;
    e.merkle_root = merkle_root;
    e.total = total;
    e.created_ts = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn claim_rewards(ctx: Context<ClaimRewards>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
    require!(amount > 0, CustomErrorCode::InvalidAmount);
    // leaf = sha256(user || amount_le || epoch_index_le)
    let mut data = Vec::with_capacity(32 + 8 + 8);
    data.extend_from_slice(ctx.accounts.user.key.as_ref());
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&ctx.accounts.epoch.index.to_le_bytes());
    let mut node = hashv(&[&data]).to_bytes();

    // Merkle verify (sorted pairs)
    for sib in &proof {
        let (a, b) = if node <= *sib {
            (node, *sib)
        } else {
            (*sib, node)
        };
        node = hashv(&[&a, &b]).to_bytes();
    }
    require!(
        node == ctx.accounts.epoch.merkle_root,
        CustomErrorCode::InvalidMerkleProof
    );

    // mint tokens (wYLDS) to user
    let seeds: &[&[u8]] = &[b"mint_authority", &[ctx.bumps.mint_authority]];
    let signer = &[&seeds[..]];
    let cpi_accounts = MintTo {
        mint: ctx.accounts.mint.to_account_info(),
        to: ctx.accounts.user_mint_token_account.to_account_info(),
        authority: ctx.accounts.mint_authority.to_account_info(),
    };
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        ),
        amount,
    )?;
    Ok(())
}
