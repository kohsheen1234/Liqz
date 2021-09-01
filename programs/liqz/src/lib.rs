mod nft_bid;
mod nft_deposit;
mod nft_pool;
mod utils;

pub use nft_deposit::{DepositState, LoanActiveState, LoanRepayedState};

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};
use fehler::throw;
use solana_program::pubkey::Pubkey;
use std::u64;

pub trait DerivedAccountIdentifier {
    const SEED: &'static [u8];
}

// The contract account should have address find_program_address(&[seed], program_id)
#[account]
#[derive(Debug)]
pub struct NFTPool {
    pub bump_seed: u8,
    pub owner: Pubkey,
    pub liz_mint: Pubkey,
    pub tai_mint: Pubkey,
    pub dai_mint: Pubkey,
    pub incentive: u64,         // incentive amount when user mortgage their NFT
    pub max_loan_duration: i64, // max loan duration before liquidation, secs
    pub service_fee_rate: u64,  // in bp, one ten thousandth, fee rate charged by liqz
    pub interest_rate: u64,     // in bp, one ten thousandth
    pub mortgage_rate: u64,     // in bp, mortgage rate to calculate real borrow amount
}

#[account]
#[derive(Debug)]
pub struct NFTBid {
    pub price: u64, // DAI Price
    pub qty: u64,
}

// One NFTDeposit corresponds to one token
#[account]
#[derive(Debug)]
pub struct NFTDeposit {
    deposit_id: Pubkey,
    state: DepositState,
}

#[program]
pub mod liqz {
    use super::*;

    pub fn initialize(ctx: Context<AccountsInitialize>) -> Result<()> {
        let AccountsInitialize {
            pool_owner,
            pool,

            liz_mint,
            tai_mint,
            dai_mint,

            pool_liz_account,
            pool_tai_account,
            pool_dai_account,

            ata_program,
            spl_program,
            system_program: system,
            rent,
        } = ctx.accounts;

        let pool = NFTPool::new_checked(
            ctx.program_id,
            pool,
            pool_owner,
            liz_mint,
            tai_mint,
            dai_mint,
            rent,
            system,
        )?;

        // Create token accounts for this contract for liz, TAI and DAI
        for (mint, token) in &[
            (liz_mint, pool_liz_account),
            (tai_mint, pool_tai_account),
            (dai_mint, pool_dai_account),
        ] {
            utils::create_associated_token_account(
                &pool.to_account_info(),
                pool_owner,
                mint,
                token,
                ata_program,
                spl_program,
                system,
                rent,
            )?;
        }

        emit!(EventInitialized {
            account: *pool.to_account_info().key
        });

        Ok(())
    }

    pub fn change_loan_settings(
        ctx: Context<AccountsChangeLoanSetting>,
        incentive: Option<u64>,
        interest_rate: Option<u64>,
        service_fee_rate: Option<u64>,
        max_loan_duration: Option<i64>,
        mortgage_rate: Option<u64>,
    ) -> Result<()> {
        let pool = &mut ctx.accounts.pool;

        incentive.map(|v| pool.incentive = v);
        interest_rate.map(|v| pool.interest_rate = v);
        service_fee_rate.map(|v| pool.service_fee_rate = v);
        max_loan_duration.map(|v| pool.max_loan_duration = v);
        mortgage_rate.map(|v| pool.mortgage_rate = v);

        emit!(EventLoanSettingChanged {
            incentive: pool.incentive,
            interest_rate: pool.interest_rate,
            service_fee_rate: pool.service_fee_rate,
            max_loan_duration: pool.max_loan_duration,
            mortgage_rate: pool.mortgage_rate,
        });
        Ok(())
    }

    // Deposits NFT asset into the pool, creating an entry of NFTListing
    pub fn deposit_nft(ctx: Context<AccountsDepositNFT>, deposit_id: Pubkey) -> Result<()> {
        let AccountsDepositNFT {
            pool,
            borrower_wallet_account,

            nft_mint,
            liz_mint,

            pool_nft_account,
            borrower_nft_account,

            pool_liz_account,
            borrower_liz_account,

            deposit_account,

            rent,

            ata_program,
            spl_program,
            system_program,
        } = ctx.accounts;

        assert_eq!(liz_mint.to_account_info().key, &pool.liz_mint);
        assert_eq!(pool_liz_account.mint, pool.liz_mint);
        assert_eq!(nft_mint.decimals, 0);

        // allocate the NFT ATA for the pool if not allocated
        NFTPool::ensure_pool_token_account(
            pool,
            nft_mint,
            pool_nft_account,
            borrower_wallet_account,
            ata_program,
            spl_program,
            system_program,
            rent,
        )?;

        // allocate the liz ATA for the user if not allocated
        NFTPool::ensure_user_token_account(
            borrower_wallet_account,
            liz_mint,
            borrower_liz_account,
            ata_program,
            spl_program,
            system_program,
            rent,
        )?;

        // create and deposit to the deposit account
        // error out if the account exists
        let deposit_account = NFTDeposit::deposit(
            ctx.program_id,
            &deposit_id,
            nft_mint.to_account_info().key,
            borrower_wallet_account,
            deposit_account,
            rent,
            system_program,
        )?;

        // Transfer NFT to the pool
        anchor_spl::token::transfer(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: borrower_nft_account.to_account_info(),
                    to: pool_nft_account.clone(),
                    authority: borrower_wallet_account.clone(),
                },
            ),
            1,
        )?;

        // Transfer incentive liz to the borrower
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_liz_account.to_account_info(),
                    to: borrower_liz_account.clone(),
                    authority: pool.to_account_info(),
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            pool.incentive,
        )?;

        // Persistent back the data. Since we created the ProgramAccount by ourselves, we need to do this manually.
        deposit_account.exit(ctx.program_id)?;

        emit!(EventNFTDeposited {
            mint: *nft_mint.to_account_info().key,
            from: *borrower_wallet_account.key,
        });

        Ok(())
    }

    // withdraw the deposited NFT
    pub fn withdraw_nft(ctx: Context<AccountsWithdrawNFT>, deposit_id: Pubkey) -> Result<()> {
        // TODO: Do we set the minimal nft lock in time?
        let AccountsWithdrawNFT {
            pool,
            borrower_wallet_account,
            nft_mint,
            borrower_nft_account,
            pool_nft_account,
            deposit_account,
            spl_program,
        } = ctx.accounts;

        // verify the deposit account indeed belongs to the user
        let (_, bump) = NFTDeposit::get_address_with_bump(
            ctx.program_id,
            nft_mint.to_account_info().key,
            borrower_wallet_account.key,
            &deposit_id,
        );
        NFTDeposit::verify_address(
            ctx.program_id,
            nft_mint.to_account_info().key,
            borrower_wallet_account.key,
            &deposit_id,
            bump,
            deposit_account.to_account_info().key,
        )?;

        // withdraw also verifies the count
        deposit_account.withdraw()?;

        // transfer the NFT back to the user
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_nft_account.to_account_info(),
                    to: borrower_nft_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            1,
        )?;

        emit!(EventNFTWithdrawn {
            mint: *nft_mint.to_account_info().key,
            to: *borrower_wallet_account.key,
        });

        Ok(())
    }

    pub fn place_bid(ctx: Context<AccountsPlaceBid>, price: u64, qty: u64) -> Result<()> {
        if qty == 0 {
            return Ok(());
        }

        let AccountsPlaceBid {
            pool,
            lender_wallet_account,
            nft_mint,
            lender_dai_account,
            bid_account,
            spl_program,
            system_program,
            rent,
        } = ctx.accounts;

        if qty > nft_mint.supply {
            throw!(liqzError::NFTBidQtyLargerThanSupply);
        }

        assert_eq!(nft_mint.decimals, 0);

        anchor_spl::token::approve(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Approve {
                    to: lender_dai_account.to_account_info(),
                    delegate: pool.to_account_info(),
                    authority: lender_wallet_account.to_account_info(),
                },
            ),
            price * qty,
        )?;

        // create the bid account if not created
        let mut bid_account = NFTBid::ensure(
            ctx.program_id,
            nft_mint.to_account_info().key,
            lender_wallet_account,
            bid_account,
            rent,
            system_program,
        )?;
        bid_account.set(price, qty);

        // Persistent back the data. Since we created the ProgramAccount by ourselves, we need to do this manually.
        bid_account.exit(ctx.program_id)?;

        emit!(EventNFTBidPlaced {
            mint: *nft_mint.to_account_info().key,
            from: *lender_wallet_account.key,
            price,
            qty,
        });

        Ok(())
    }

    pub fn cancel_bid(ctx: Context<AccountsCancelBid>, revoke: bool) -> Result<()> {
        let AccountsCancelBid {
            lender_wallet_account,
            nft_mint,
            lender_dai_account,
            bid_account,
            spl_program,
        } = ctx.accounts;

        assert_eq!(nft_mint.decimals, 0);

        let (_, bump) = NFTBid::get_address_with_bump(
            ctx.program_id,
            nft_mint.to_account_info().key,
            lender_wallet_account.key,
        );

        NFTBid::verify_address(
            ctx.program_id,
            nft_mint.to_account_info().key,
            lender_wallet_account.key,
            bump,
            bid_account.to_account_info().key,
        )?;

        emit!(EventNFTBidCancelled {
            mint: *nft_mint.to_account_info().key,
            from: *lender_wallet_account.key,
            price: bid_account.price,
            qty: bid_account.qty,
        });

        bid_account.cancel();

        if revoke {
            solana_program::program::invoke(
                &spl_token::instruction::revoke(
                    &spl_token::id(),
                    lender_dai_account.to_account_info().key,
                    lender_wallet_account.to_account_info().key,
                    &[lender_wallet_account.key],
                )?,
                &[
                    lender_dai_account.to_account_info(),
                    lender_wallet_account.to_account_info(),
                    spl_program.clone(),
                ],
            )?;
        }

        Ok(())
    }

    pub fn borrow(ctx: Context<AccountsBorrow>, amount: u64) -> Result<()> {
        let AccountsBorrow {
            pool,
            borrower_wallet_account,
            lender_wallet_account,

            nft_mint,

            pool_dai_account,
            borrower_dai_account,
            lender_dai_account,

            pool_tai_account,
            lender_tai_account,

            bid_account,
            deposit_account,

            spl_program,
            clock,
        } = ctx.accounts;

        if amount > bid_account.price {
            throw!(liqzError::NFTBorrowExceedBidAmount)
        }

        assert_eq!(lender_tai_account.mint, pool.tai_mint);
        assert_eq!(pool_tai_account.mint, pool.tai_mint);
        assert_eq!(lender_dai_account.mint, pool.dai_mint);
        assert_eq!(borrower_dai_account.mint, pool.dai_mint);

        let (_, bump) = NFTDeposit::get_address_with_bump(
            ctx.program_id,
            nft_mint.to_account_info().key,
            borrower_wallet_account.key,
            &deposit_account.deposit_id,
        );

        NFTDeposit::verify_address(
            ctx.program_id,
            nft_mint.to_account_info().key,
            borrower_wallet_account.key,
            &deposit_account.deposit_id,
            bump,
            deposit_account.to_account_info().key,
        )?;

        // set related records
        let total_amount = amount;
        let borrowed_amount = total_amount
            .checked_mul(pool.mortgage_rate)
            .unwrap()
            .checked_div(10000)
            .unwrap();

        if borrowed_amount <= 0 {
            throw!(liqzError::BorrowedAmountTooSmall)
        }

        deposit_account.start_borrow(
            *lender_wallet_account.key,
            total_amount,
            borrowed_amount,
            clock.unix_timestamp,
            pool.max_loan_duration,
        )?;

        // decrease the bid qty by 1;
        bid_account.trade(1)?;

        // transfer DAI to the pool
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: lender_dai_account.to_account_info(),
                    to: pool_dai_account.to_account_info(),
                    authority: pool.to_account_info(), // The pool is the delegate
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            total_amount,
        )?;

        // transfer DAI to the borrower
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_dai_account.to_account_info(),
                    to: borrower_dai_account.to_account_info(),
                    authority: pool.to_account_info(), // The pool is the delegate
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            borrowed_amount,
        )?;

        // transfer TAI to the lender
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_tai_account.to_account_info(),
                    to: lender_tai_account.to_account_info(),
                    authority: pool.to_account_info(), // The pool is the delegate
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            borrowed_amount,
        )?;

        emit!(EventBorrowed {
            borrower: *borrower_wallet_account.key,
            lender: *lender_wallet_account.key,
            amount: borrowed_amount,
            length: pool.max_loan_duration
        });

        Ok(())
    }

    pub fn repay(ctx: Context<AccountsRepay>) -> Result<()> {
        let AccountsRepay {
            pool,
            borrower_wallet_account,
            pool_owner_dai_account,
            borrower_dai_account,
            lender_dai_account,

            borrower_nft_account,
            pool_nft_account,

            deposit_account,
            spl_program,
            clock,
        } = ctx.accounts;

        let loan = deposit_account.get_active_state()?;

        if clock.unix_timestamp > loan.expired_at {
            throw!(liqzError::LoanLiquidated)
        }

        assert!(pool_owner_dai_account.owner == pool.owner);

        let (interest, fee) = pool.calculate_interest_and_fee(
            loan.borrowed_amount,
            clock.unix_timestamp.saturating_sub(loan.started_at),
        );

        // transfer fee to the owner
        anchor_spl::token::transfer(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: borrower_dai_account.to_account_info(),
                    to: pool_owner_dai_account.to_account_info(),
                    authority: borrower_wallet_account.to_account_info(),
                },
            ),
            fee,
        )?;

        let lender_income = interest.checked_sub(fee).unwrap();
        let repayed_amount = loan.total_amount.checked_add(lender_income).unwrap();

        // transfer the DAI to the pool, waiting for the lender to withdraw
        anchor_spl::token::transfer(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: borrower_dai_account.to_account_info(),
                    to: lender_dai_account.to_account_info(),
                    authority: borrower_wallet_account.to_account_info(),
                },
            ),
            repayed_amount,
        )?;

        // transfer the NFT to the borrower
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_nft_account.to_account_info(),
                    to: borrower_nft_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            1,
        )?;

        // set corresponding records
        deposit_account.repay(loan.borrowed_amount.checked_add(lender_income).unwrap())?;

        emit!(EventRepayed {
            borrower: *borrower_wallet_account.key,
            lender: loan.lender,
            amount: repayed_amount,
            fee,
            lender_income
        });

        Ok(())
    }

    pub fn liquidate(ctx: Context<AccountsLiquidate>) -> Result<()> {
        let AccountsLiquidate {
            pool,
            lender_wallet_account,

            pool_owner_dai_account,
            pool_dai_account,
            lender_dai_account,

            nft_mint,
            pool_nft_account,
            lender_nft_account,

            lender_tai_account,
            pool_tai_account,

            deposit_account,

            ata_program,
            spl_program,
            system_program,
            rent,
            clock,
        } = ctx.accounts;

        let loan = deposit_account.get_active_state()?;

        if clock.unix_timestamp <= loan.expired_at {
            throw!(liqzError::LoanNotExpired)
        }

        // Transfer the corresponding TAI to the pool
        anchor_spl::token::transfer(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: lender_tai_account.to_account_info(),
                    to: pool_tai_account.to_account_info(),
                    authority: lender_wallet_account.to_account_info(),
                },
            ),
            loan.borrowed_amount,
        )?;

        // charge service fee using max_borrow_duration
        let (_, fee) =
            pool.calculate_interest_and_fee(loan.borrowed_amount, pool.max_loan_duration);

        // transfer fee to the owner
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_dai_account.to_account_info(),
                    to: pool_owner_dai_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            fee,
        )?;

        let withdrawable = loan.total_amount - loan.borrowed_amount - fee;

        // Transfer the remaining DAI to the lender
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_dai_account.to_account_info(),
                    to: lender_dai_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            withdrawable,
        )?;

        // allocate the NFT ATA for the lender if not allocate
        NFTPool::ensure_user_token_account(
            lender_wallet_account,
            nft_mint,
            lender_nft_account,
            ata_program,
            spl_program,
            system_program,
            rent,
        )?;

        // Transfer the NFT to the lender
        anchor_spl::token::transfer(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_nft_account.to_account_info(),
                    to: lender_nft_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
            ),
            1,
        )?;

        // set corresponding records
        deposit_account.liquidate()?;

        emit!(EventLiquidated {
            lender: *lender_wallet_account.key,
            loan_id: deposit_account.deposit_id,
            withdrawable,
        });

        Ok(())
    }

    pub fn withdraw_locked_asset(ctx: Context<AccountsWithdrawLockedAsset>) -> Result<()> {
        let AccountsWithdrawLockedAsset {
            pool,
            lender_wallet_account,

            lender_tai_account,
            pool_tai_account,

            lender_dai_account,
            pool_dai_account,

            deposit_account,

            spl_program,
        } = ctx.accounts;

        let repay = deposit_account.get_repayed_state()?;

        // Transfer the TAI to the pool
        anchor_spl::token::transfer(
            CpiContext::new(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: lender_tai_account.to_account_info(),
                    to: pool_tai_account.to_account_info(),
                    authority: lender_wallet_account.to_account_info(),
                },
            ),
            repay.tai_required_to_unlock,
        )?;

        // Transfer the DAI to the lender
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                spl_program.clone(),
                anchor_spl::token::Transfer {
                    from: pool_dai_account.to_account_info(),
                    to: lender_dai_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[&[NFTPool::SEED, &[pool.bump_seed]]],
            ),
            repay.lender_withdrawable,
        )?;

        deposit_account.clear()?;

        emit!(EventWithDrawLockedAsset {
            lender: *lender_wallet_account.key,
            amount: repay.lender_withdrawable,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct AccountsInitialize<'info> {
    #[account(signer)]
    pub pool_owner: AccountInfo<'info>, // also the funder and the fee collector
    #[account(mut)]
    pub pool: AccountInfo<'info>, // We cannot use  ProgramAccount<'info, liqzContract> here because it is not allocated yet

    pub liz_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub pool_liz_account: AccountInfo<'info>, // this is not allocated yet

    pub tai_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub pool_tai_account: AccountInfo<'info>, // this is not allocated yet

    pub dai_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub pool_dai_account: AccountInfo<'info>, // this is not allocated yet

    pub ata_program: AccountInfo<'info>,
    pub spl_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
}
#[derive(Accounts)]
pub struct AccountsChangeLoanSetting<'info> {
    #[account(signer)]
    pub owner: AccountInfo<'info>, // only owner can change the setting
    #[account(mut, has_one = owner)]
    pub pool: ProgramAccount<'info, NFTPool>,
}

#[derive(Accounts)]
pub struct AccountsDepositNFT<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,
    #[account(signer)]
    pub borrower_wallet_account: AccountInfo<'info>,

    pub nft_mint: CpiAccount<'info, Mint>,
    pub liz_mint: CpiAccount<'info, Mint>,

    #[account(mut)]
    pub borrower_nft_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_nft_account: AccountInfo<'info>, // potentially this is not allocated yet

    #[account(mut)]
    pub borrower_liz_account: AccountInfo<'info>, // potentially this is not allocated yet
    #[account(mut)]
    pub pool_liz_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub deposit_account: AccountInfo<'info>, // Essentially this is ProgramAccount<NFTDeposit>, however, we've not allocated the space for it yet. We cannot use ProgramAccount here.

    pub rent: Sysvar<'info, Rent>,

    pub ata_program: AccountInfo<'info>,
    pub spl_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountsWithdrawNFT<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,
    #[account(signer)]
    pub borrower_wallet_account: AccountInfo<'info>,

    pub nft_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub pool_nft_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub borrower_nft_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub deposit_account: ProgramAccount<'info, NFTDeposit>,

    pub spl_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountsPlaceBid<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,
    #[account(signer)]
    pub lender_wallet_account: AccountInfo<'info>,

    pub nft_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub lender_dai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub bid_account: AccountInfo<'info>, // Essentially this is ProgramAccount<NFTBid>, however, we've not allocated the space for it yet. We cannot use ProgramAccount here.

    pub spl_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct AccountsCancelBid<'info> {
    #[account(signer)]
    pub lender_wallet_account: AccountInfo<'info>,

    pub nft_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub lender_dai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub bid_account: ProgramAccount<'info, NFTBid>,

    pub spl_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AccountsBorrow<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,
    #[account(signer)]
    pub borrower_wallet_account: AccountInfo<'info>,
    pub lender_wallet_account: AccountInfo<'info>,

    pub nft_mint: CpiAccount<'info, Mint>,

    #[account(mut)]
    pub pool_dai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub borrower_dai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub lender_dai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub lender_tai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_tai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub deposit_account: ProgramAccount<'info, NFTDeposit>,
    #[account(mut)]
    pub bid_account: ProgramAccount<'info, NFTBid>,

    pub spl_program: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct AccountsRepay<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,

    #[account(signer)]
    pub borrower_wallet_account: AccountInfo<'info>,

    #[account(mut)]
    pub pool_owner_dai_account: CpiAccount<'info, TokenAccount>, // for collecting fees
    #[account(mut)]
    pub borrower_dai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub lender_dai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub borrower_nft_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_nft_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub deposit_account: ProgramAccount<'info, NFTDeposit>,

    #[account(mut)]
    pub spl_program: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct AccountsLiquidate<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,
    #[account(signer)]
    pub lender_wallet_account: AccountInfo<'info>,

    #[account(mut)]
    pub pool_dai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_owner_dai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub lender_dai_account: CpiAccount<'info, TokenAccount>,

    pub nft_mint: CpiAccount<'info, Mint>,
    #[account(mut)]
    pub pool_nft_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub lender_nft_account: AccountInfo<'info>, // Possibly not allocated

    #[account(mut)]
    pub lender_tai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_tai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub deposit_account: ProgramAccount<'info, NFTDeposit>,

    pub ata_program: AccountInfo<'info>,
    pub spl_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct AccountsWithdrawLockedAsset<'info> {
    pub pool: ProgramAccount<'info, NFTPool>,

    #[account(signer)]
    pub lender_wallet_account: AccountInfo<'info>,

    #[account(mut)]
    pub lender_tai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_tai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub lender_dai_account: CpiAccount<'info, TokenAccount>,
    #[account(mut)]
    pub pool_dai_account: CpiAccount<'info, TokenAccount>,

    #[account(mut)]
    pub deposit_account: ProgramAccount<'info, NFTDeposit>,

    pub spl_program: AccountInfo<'info>,
}

#[error]
pub enum liqzError {
    #[msg("Not Authorized")]
    NotAuhorized = 0,
    #[msg("Contract address not correct")]
    ContractAddressNotCorrect,

    #[msg("NFT listing address not correct")]
    NFTListingAddressNotCorrect,

    #[msg("NFT bid address not correct")]
    NFTBidAddressNotCorrect,

    #[msg("NFT loan address not correct")]
    NFTLoanAddressNotCorrect,

    #[msg("NFT overdrawn")]
    NFTOverdrawn,

    #[msg("Empty NFT Reserve")]
    EmptyNFTReserve,

    #[msg("NFT overtrade")]
    NFTOvertrade,

    #[msg("NFT bid qty larger than NFT supply")]
    NFTBidQtyLargerThanSupply,

    #[msg("NFT borrow amount larger than bid amount")]
    NFTBorrowExceedBidAmount,

    #[msg("NFT borrow already started")]
    BorrowAlreadyStarted,

    #[msg("Loan is liquidated")]
    LoanLiquidated,

    #[msg("Loan is not expired yet")]
    LoanNotExpired,

    #[msg("Loan record already exist")]
    LoanAlreadyExist,

    #[msg("Loan already finalized")]
    LoanFinialized,

    #[msg("Loan is not active")]
    LoanNotActive,

    #[msg("Not enough NFT in pool")]
    NotEnoughNFTInPool,

    #[msg("NFT already withdrawn")]
    NFTAlreadyWithdrawn,

    #[msg("NFT is locked")]
    NFTLocked,

    #[msg("The borrowed amount is too small")]
    BorrowedAmountTooSmall,

    #[msg("Loan has not been repayed")]
    LoanNotRepayed,
}

impl liqzError {
    pub fn from_code(c: u32) -> liqzError {
        unsafe { std::mem::transmute(c - 100) }
    }
}

#[event]
#[derive(Debug)]
pub struct EventInitialized {
    account: Pubkey,
}

#[event]
#[derive(Debug)]
pub struct EventLoanSettingChanged {
    incentive: u64,
    interest_rate: u64,
    service_fee_rate: u64,
    max_loan_duration: i64,
    mortgage_rate: u64,
}

#[event]
#[derive(Debug)]
pub struct EventNFTDeposited {
    mint: Pubkey,
    from: Pubkey,
}

#[event]
#[derive(Debug)]
pub struct EventNFTWithdrawn {
    mint: Pubkey,
    to: Pubkey,
}

#[event]
#[derive(Debug)]
pub struct EventNFTBidPlaced {
    mint: Pubkey,
    from: Pubkey,
    price: u64,
    qty: u64,
}

#[event]
#[derive(Debug)]
pub struct EventNFTBidCancelled {
    mint: Pubkey,
    from: Pubkey,
    price: u64,
    qty: u64,
}

#[event]
#[derive(Debug)]
pub struct EventBorrowed {
    borrower: Pubkey,
    lender: Pubkey,
    amount: u64,
    length: i64,
}

#[event]
#[derive(Debug)]
pub struct EventRepayed {
    borrower: Pubkey,
    lender: Pubkey,
    amount: u64,
    fee: u64,
    lender_income: u64,
}

#[event]
#[derive(Debug)]
pub struct EventLiquidated {
    lender: Pubkey,
    loan_id: Pubkey,
    withdrawable: u64,
}

#[event]
#[derive(Debug)]
pub struct EventWithDrawLockedAsset {
    lender: Pubkey,
    amount: u64,
}
