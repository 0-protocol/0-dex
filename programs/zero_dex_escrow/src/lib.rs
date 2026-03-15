use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("9xQeWvG816bUx9EPjHmaT23yvVMv4A5M4xRjPj7j9VYV");

#[program]
pub mod zero_dex_escrow {
    use super::*;

    pub fn execute_swap(
        ctx: Context<ExecuteSwap>,
        amount_a: u64,
        amount_b: u64,
        _match_proof_hash: [u8; 32],
    ) -> Result<()> {
        require!(
            cfg!(feature = "solana-experimental"),
            ErrorCode::ProgramDisabled
        );
        require!(amount_a > 0 && amount_b > 0, ErrorCode::InvalidAmount);

        let cpi_accounts_a = Transfer {
            from: ctx.accounts.party_a_token_a.to_account_info(),
            to: ctx.accounts.party_b_token_a.to_account_info(),
            authority: ctx.accounts.party_a.to_account_info(),
        };
        let cpi_program_a = ctx.accounts.token_program.to_account_info();
        let cpi_ctx_a = CpiContext::new(cpi_program_a, cpi_accounts_a);
        token::transfer(cpi_ctx_a, amount_a)?;

        let cpi_accounts_b = Transfer {
            from: ctx.accounts.party_b_token_b.to_account_info(),
            to: ctx.accounts.party_a_token_b.to_account_info(),
            authority: ctx.accounts.party_b.to_account_info(),
        };
        let cpi_program_b = ctx.accounts.token_program.to_account_info();
        let cpi_ctx_b = CpiContext::new(cpi_program_b, cpi_accounts_b);
        token::transfer(cpi_ctx_b, amount_b)?;

        msg!("0-dex Agent Trade Settled: {} A for {} B", amount_a, amount_b);

        Ok(())
    }
}

#[derive(Accounts)]
pub struct ExecuteSwap<'info> {
    #[account(mut)]
    pub party_a: Signer<'info>,
    #[account(mut)]
    pub party_b: Signer<'info>,

    #[account(
        mut,
        constraint = party_a_token_a.owner == party_a.key() @ ErrorCode::InvalidTokenOwner,
        constraint = party_a_token_a.mint == token_a_mint.key() @ ErrorCode::InvalidMatchProof
    )]
    pub party_a_token_a: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = party_a_token_b.owner == party_a.key() @ ErrorCode::InvalidTokenOwner,
        constraint = party_a_token_b.mint == token_b_mint.key() @ ErrorCode::InvalidMatchProof
    )]
    pub party_a_token_b: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = party_b_token_a.owner == party_b.key() @ ErrorCode::InvalidTokenOwner,
        constraint = party_b_token_a.mint == token_a_mint.key() @ ErrorCode::InvalidMatchProof
    )]
    pub party_b_token_a: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = party_b_token_b.owner == party_b.key() @ ErrorCode::InvalidTokenOwner,
        constraint = party_b_token_b.mint == token_b_mint.key() @ ErrorCode::InvalidMatchProof
    )]
    pub party_b_token_b: Account<'info, TokenAccount>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Solana settlement path is disabled for production launch scope.")]
    ProgramDisabled,
    #[msg("Transfer amount must be non-zero.")]
    InvalidAmount,
    #[msg("Provided token account owner does not match signer.")]
    InvalidTokenOwner,
    #[msg("Invalid Tensor Match Proof.")]
    InvalidMatchProof,
}
