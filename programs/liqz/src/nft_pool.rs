use crate::{utils, DerivedAccountIdentifier, NFTPool, liqzError};
use anchor_lang::prelude::*;
use anchor_spl::token::Mint;
use fehler::{throw, throws};
use std::convert::TryInto;

static SECONDS_PER_DAY: u64 = 24 * 60 * 60;

type Result<T> = std::result::Result<T, ProgramError>;

impl DerivedAccountIdentifier for NFTPool {
    const SEED: &'static [u8] = b"liqzNFTPool";
}

impl NFTPool {
    #[throws(ProgramError)]
    pub fn new_checked<'info>(
        program_id: &Pubkey,
        pool: &AccountInfo<'info>,
        pool_owner: &AccountInfo<'info>,
        liz_mint: &CpiAccount<'info, Mint>,
        tai_mint: &CpiAccount<'info, Mint>,
        dai_mint: &CpiAccount<'info, Mint>,
        rent: &Sysvar<'info, Rent>,
        system_program: &AccountInfo<'info>,
    ) -> ProgramAccount<'info, Self> {
        let (_, bump) = NFTPool::get_address_with_bump(program_id);
        NFTPool::verify_address(program_id, bump, &pool.key)?;

        let instance = Self {
            bump_seed: bump,
            owner: *pool_owner.key,
            liz_mint: *liz_mint.to_account_info().key,
            tai_mint: *tai_mint.to_account_info().key,
            dai_mint: *dai_mint.to_account_info().key,
            incentive: 100 * 10u64.pow(liz_mint.decimals as u32) as u64,
            max_loan_duration: 30 * 24 * 60 * 60, // 30 days
            // 5%
            service_fee_rate: 500,
            // 1%
            interest_rate: 100,
            // 90%
            mortgage_rate: 9000,
        };

        let acc_size = 8 + instance
            .try_to_vec()
            .map_err(|_| ProgramError::Custom(1))?
            .len() as u64;

        // allocate the space for the contract account
        utils::create_derived_account_with_seed(
            program_id, // The program ID of liqz Contract
            &pool_owner,
            &[Self::SEED, &[bump]],
            &pool,
            acc_size,
            &rent,
            &system_program,
        )?;

        // let the data borrow invalid after exiting the scope. Otherwise can cannot borrow it again in the ProgramAccount::try_from
        {
            let mut data = pool.try_borrow_mut_data()?;
            let mut cursor = std::io::Cursor::new(&mut **data);
            instance.try_serialize(&mut cursor)?;
        }

        ProgramAccount::try_from(pool)?
    }

    pub fn ensure_pool_token_account<'info>(
        pool: &ProgramAccount<'info, NFTPool>,
        mint: &CpiAccount<'info, Mint>,
        pool_token_account: &AccountInfo<'info>,
        user_wallet_account: &AccountInfo<'info>,
        ata_program: &AccountInfo<'info>,
        spl_program: &AccountInfo<'info>,
        system: &AccountInfo<'info>,
        rent: &Sysvar<'info, Rent>,
    ) -> Result<()> {
        if !utils::is_account_allocated(pool_token_account) {
            utils::create_associated_token_account(
                &pool.to_account_info(),
                user_wallet_account,
                mint,
                pool_token_account,
                ata_program,
                spl_program,
                system,
                rent,
            )?;
        }

        Ok(())
    }

    pub fn ensure_user_token_account<'info>(
        user_wallet_account: &AccountInfo<'info>,
        mint: &CpiAccount<'info, Mint>,
        user_token_account: &AccountInfo<'info>,
        ata_program: &AccountInfo<'info>,
        spl_program: &AccountInfo<'info>,
        system: &AccountInfo<'info>,
        rent: &Sysvar<'info, Rent>,
    ) -> Result<()> {
        if !utils::is_account_allocated(user_token_account) {
            utils::create_associated_token_account(
                user_wallet_account,
                user_wallet_account,
                mint,
                user_token_account,
                ata_program,
                spl_program,
                system,
                rent,
            )?;
        }

        Ok(())
    }

    pub fn calculate_interest_and_fee(&self, borrowed_amount: u64, duration: i64) -> (u64, u64) {
        let interest = borrowed_amount
            .checked_mul(self.interest_rate)
            .unwrap()
            .checked_mul(duration.try_into().unwrap())
            .unwrap()
            .checked_div(SECONDS_PER_DAY)
            .unwrap()
            .checked_div(10000)
            .unwrap();
        let fee = interest
            .checked_mul(self.service_fee_rate)
            .unwrap()
            .checked_div(10000)
            .unwrap();

        (interest, fee)
    }
    pub fn get_address(program_id: &Pubkey) -> Pubkey {
        Self::get_address_with_bump(program_id).0
    }

    pub(crate) fn get_address_with_bump(program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[Self::SEED], program_id)
    }

    #[throws(ProgramError)]
    pub fn verify_address(program_id: &Pubkey, bump: u8, pool_address: &Pubkey) {
        let addr = Pubkey::create_program_address(&[Self::SEED, &[bump]], program_id)?;

        if &addr != pool_address {
            throw!(liqzError::ContractAddressNotCorrect);
        }
    }
}
