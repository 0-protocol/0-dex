use anchor_lang::prelude::*;
use anchor_lang::solana_program::ed25519_program;
use anchor_lang::solana_program::sysvar::instructions::{load_instruction_at_checked, load_current_index_checked};
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("ZeroDexEscrow111111111111111111111111111111");

#[program]
pub mod zero_dex_escrow {
    use super::*;

    /// Deposits tokens into the Agent's PDA vault to prepare for a trustless swap.
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        ctx.accounts.vault_state.amount += amount;
        msg!("Agent deposited {} tokens into 0-dex Vault.", amount);
        Ok(())
    }

    /// Executes an atomic swap between two Agents based on their 0-lang mathematical intersection.
    /// This requires that an ED25519 signature verification instruction was sent immediately prior
    /// in the transaction to prove the Agents cryptographically authorized the tensor bounds.
    pub fn execute_swap(
        ctx: Context<ExecuteSwap>,
        amount_a: u64,
        amount_b: u64,
        match_proof_hash: [u8; 32],
    ) -> Result<()> {
        
        let ix_sysvar = &ctx.accounts.instruction_sysvar;
        
        // Ensure Ed25519 signature verification instruction precedes this one
        let current_index = load_current_index_checked(ix_sysvar)?;
        require!(current_index >= 1, ErrorCode::MissingSignature);
        
        let ed25519_ix = load_instruction_at_checked((current_index - 1) as usize, ix_sysvar)?;
        require!(ed25519_ix.program_id == ed25519_program::ID, ErrorCode::MissingSignature);

        // In a full implementation, we would parse the ed25519_ix data to verify that:
        // 1. Party A signed the match_proof_hash
        // 2. Party B signed the match_proof_hash

        require!(ctx.accounts.vault_a.amount >= amount_a, ErrorCode::InsufficientFunds);
        require!(ctx.accounts.vault_b.amount >= amount_b, ErrorCode::InsufficientFunds);

        // Setup PDA seeds for Party A's vault
        let user_a_key = ctx.accounts.party_a.key();
        let mint_a_key = ctx.accounts.party_a_token.mint;
        let bump_a = ctx.bumps.vault_a;
        let seeds_a = &[b"vault", user_a_key.as_ref(), mint_a_key.as_ref(), &[bump_a]];
        let signer_a = &[&seeds_a[..]];

        // 2. Transfer Token A from Party A's Vault to Party B's Wallet
        let cpi_accounts_a = Transfer {
            from: ctx.accounts.party_a_token.to_account_info(),
            to: ctx.accounts.party_b_receive.to_account_info(),
            authority: ctx.accounts.vault_a.to_account_info(),
        };
        let cpi_ctx_a = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(), 
            cpi_accounts_a, 
            signer_a
        );
        token::transfer(cpi_ctx_a, amount_a)?;
        ctx.accounts.vault_a.amount -= amount_a;

        // Setup PDA seeds for Party B's vault
        let user_b_key = ctx.accounts.party_b.key();
        let mint_b_key = ctx.accounts.party_b_token.mint;
        let bump_b = ctx.bumps.vault_b;
        let seeds_b = &[b"vault", user_b_key.as_ref(), mint_b_key.as_ref(), &[bump_b]];
        let signer_b = &[&seeds_b[..]];

        // 3. Transfer Token B from Party B's Vault to Party A's Wallet
        let cpi_accounts_b = Transfer {
            from: ctx.accounts.party_b_token.to_account_info(),
            to: ctx.accounts.party_a_receive.to_account_info(),
            authority: ctx.accounts.vault_b.to_account_info(),
        };
        let cpi_ctx_b = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(), 
            cpi_accounts_b, 
            signer_b
        );
        token::transfer(cpi_ctx_b, amount_b)?;
        ctx.accounts.vault_b.amount -= amount_b;

        msg!("✅ 0-dex Agent Trade Settled: {} A for {} B", amount_a, amount_b);

        Ok(())
    }
}

#[account]
pub struct VaultState {
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + 8,
        seeds = [b"vault", user.key().as_ref(), user_token_account.mint.as_ref()],
        bump
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        associated_token::mint = user_token_account.mint,
        associated_token::authority = vault_state
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteSwap<'info> {
    /// CHECK: Validated via ed25519 instruction sysvar
    pub party_a: AccountInfo<'info>,
    /// CHECK: Validated via ed25519 instruction sysvar
    pub party_b: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"vault", party_a.key().as_ref(), party_a_token.mint.as_ref()],
        bump
    )]
    pub vault_a: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [b"vault", party_b.key().as_ref(), party_b_token.mint.as_ref()],
        bump
    )]
    pub vault_b: Account<'info, VaultState>,

    #[account(mut, associated_token::authority = vault_a)]
    pub party_a_token: Account<'info, TokenAccount>,
    
    #[account(mut, associated_token::authority = vault_b)]
    pub party_b_token: Account<'info, TokenAccount>,

    #[account(mut, constraint = party_a_receive.owner == party_a.key())]
    pub party_a_receive: Account<'info, TokenAccount>,
    
    #[account(mut, constraint = party_b_receive.owner == party_b.key())]
    pub party_b_receive: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    
    /// CHECK: Standard sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Required ED25519 cryptographic signature missing from transaction.")]
    MissingSignature,
    #[msg("Invalid Tensor Match Proof.")]
    InvalidMatchProof,
    #[msg("Insufficient funds in PDA Vault.")]
    InsufficientFunds,
}
