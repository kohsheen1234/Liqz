use anchor_lang::prelude::Pubkey;
use solana_program::clock::UnixTimestamp;

use crate::{utils, DerivedAccountIdentifier, NFTDeposit, liqzError};
use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use fehler::{throw, throws};

impl DerivedAccountIdentifier for NFTDeposit {
    const SEED: &'static [u8] = b"liqzNFTDeposit";
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum DepositState {
    PendingLoan,                   // Loan hasn't happened yet and the NFT is in the pool
    LoanActive(LoanActiveState),   // Loan is active
    LoanRepayed(LoanRepayedState), // Loan repayed and the NFT is withdrawn by the borrower

    // The following three are terminal state
    Withdrawn,      // Loan did not happen and the NFT is withdrawn by the borrower
    LoanLiquidated, // Loan liquidated and the NFT is withdrawn by the lender
    LoanCleared,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct LoanActiveState {
    pub total_amount: u64,
    pub borrowed_amount: u64,      // amount of dai
    pub started_at: UnixTimestamp, // in seconds
    pub expired_at: UnixTimestamp, // in seconds
    pub lender: Pubkey,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct LoanRepayedState {
    pub tai_required_to_unlock: u64,
    pub lender_withdrawable: u64,
    pub lender: Pubkey,
}

impl NFTDeposit {
    #[throws(ProgramError)]
    pub fn deposit<'info>(
        program_id: &Pubkey,
        deposit_id: &Pubkey,
        nft_mint: &Pubkey,
        borrower_wallet: &AccountInfo<'info>,
        deposit_account: &AccountInfo<'info>,
        rent: &Sysvar<'info, Rent>,
        system_program: &AccountInfo<'info>,
    ) -> ProgramAccount<'info, Self> {
        let (_, bump) =
            Self::get_address_with_bump(program_id, nft_mint, borrower_wallet.key, deposit_id);

        Self::verify_address(
            program_id,
            nft_mint,
            borrower_wallet.key,
            deposit_id,
            bump,
            deposit_account.key,
        )?;

        // Do not reuse the loan record
        // TODO: deallocate the loan record and give lamports back to the user.
        if crate::utils::is_account_allocated(deposit_account) {
            throw!(liqzError::LoanAlreadyExist);
        }

        let instance = NFTDeposit {
            deposit_id: *deposit_id,
            state: DepositState::PendingLoan,
        };

        let seeds_with_bump: &[&[_]] = &[
            Self::SEED,
            &nft_mint.to_bytes(),
            &borrower_wallet.key.to_bytes(),
            &deposit_id.to_bytes(),
            &[bump],
        ];

        utils::create_derived_account_with_seed(
            program_id,
            borrower_wallet,
            seeds_with_bump,
            deposit_account,
            Self::account_size() as u64,
            &rent,
            &system_program,
        )?;

        {
            let mut data = deposit_account.try_borrow_mut_data()?;
            let mut cursor = std::io::Cursor::new(&mut **data);
            instance.try_serialize(&mut cursor)?;
        }

        let loan_account = ProgramAccount::try_from(deposit_account)?;

        loan_account
    }

    #[throws(liqzError)]
    pub fn withdraw(&mut self) {
        use DepositState::*;

        match self.state {
            PendingLoan => self.state = DepositState::Withdrawn,
            Withdrawn | LoanRepayed { .. } | LoanCleared => throw!(liqzError::NFTAlreadyWithdrawn),
            LoanActive(_) | LoanLiquidated => throw!(liqzError::NFTLocked),
        }
    }

    #[throws(liqzError)]
    pub fn start_borrow(
        &mut self,
        lender: Pubkey,
        total_amount: u64,
        borrowed_amount: u64,
        start: UnixTimestamp,
        length: i64,
    ) {
        if !matches!(self.state, DepositState::PendingLoan) {
            throw!(liqzError::BorrowAlreadyStarted)
        }

        assert!(total_amount >= borrowed_amount);
        self.state = DepositState::LoanActive(LoanActiveState {
            lender,
            total_amount,
            borrowed_amount,            // amount of dai
            started_at: start,          // in seconds
            expired_at: start + length, // in seconds
        });
    }

    #[throws(liqzError)]
    pub fn repay(&mut self, lender_withdrawable: u64) {
        match self.state {
            DepositState::LoanActive(LoanActiveState {
                lender,
                borrowed_amount,
                ..
            }) => {
                self.state = DepositState::LoanRepayed(LoanRepayedState {
                    tai_required_to_unlock: borrowed_amount,
                    lender_withdrawable,
                    lender,
                })
            }
            _ => {
                throw!(liqzError::LoanNotActive)
            }
        }
    }

    #[throws(liqzError)]
    pub fn liquidate(&mut self) {
        match self.state {
            DepositState::LoanActive { .. } => {
                self.state = DepositState::LoanLiquidated;
            }
            _ => {
                throw!(liqzError::LoanNotActive)
            }
        }
    }

    #[throws(liqzError)]
    pub fn clear(&mut self) {
        match self.state {
            DepositState::LoanRepayed { .. } => {
                self.state = DepositState::LoanCleared;
            }
            _ => {
                throw!(liqzError::LoanNotRepayed)
            }
        }
    }

    #[throws(liqzError)]
    pub fn get_active_state(&self) -> LoanActiveState {
        match self.state {
            DepositState::PendingLoan
            | DepositState::LoanLiquidated
            | DepositState::LoanRepayed { .. }
            | DepositState::Withdrawn
            | DepositState::LoanCleared => throw!(liqzError::LoanNotActive),
            DepositState::LoanActive(s) => s,
        }
    }

    #[throws(liqzError)]
    pub fn get_repayed_state(&self) -> LoanRepayedState {
        match self.state {
            DepositState::PendingLoan
            | DepositState::LoanLiquidated
            | DepositState::Withdrawn
            | DepositState::LoanCleared => throw!(liqzError::LoanNotActive),
            DepositState::LoanActive { .. } => throw!(liqzError::LoanNotRepayed),
            DepositState::LoanRepayed(r) => r,
        }
    }

    // An program derived account that stores nft loan
    // The address of the account is computed as follow:
    // address = find_program_address([NFTLoan::SEED, nft_mint_address, borrower_wallet_address, loan_id], program_id)
    // only the liqz_contract_address can change the data in this account
    pub fn get_address(
        program_id: &Pubkey,
        nft_mint: &Pubkey,
        borrower_wallet: &Pubkey,
        deposit_id: &Pubkey,
    ) -> Pubkey {
        Self::get_address_with_bump(program_id, nft_mint, borrower_wallet, deposit_id).0
    }

    pub(crate) fn get_address_with_bump(
        program_id: &Pubkey,
        nft_mint: &Pubkey,
        borrower_wallet: &Pubkey,
        deposit_id: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                Self::SEED,
                &nft_mint.to_bytes(),
                &borrower_wallet.to_bytes(),
                &deposit_id.to_bytes(),
            ],
            program_id,
        )
    }

    #[throws(ProgramError)]
    pub fn verify_address(
        program_id: &Pubkey,
        nft_mint: &Pubkey,
        borrower_wallet: &Pubkey,
        deposit_id: &Pubkey,
        bump: u8,
        address: &Pubkey,
    ) {
        let addr = Pubkey::create_program_address(
            &[
                Self::SEED,
                &nft_mint.to_bytes(),
                &borrower_wallet.to_bytes(),
                &deposit_id.to_bytes(),
                &[bump],
            ],
            program_id,
        )?;

        if &addr != address {
            throw!(liqzError::NFTLoanAddressNotCorrect);
        }
    }

    fn account_size() -> usize {
        // Borsh does not support vary size structure.
        // Pick the largest variant so that we are safe
        let largest_instance = NFTDeposit {
            deposit_id: Pubkey::new(&[0u8; 32]),
            state: DepositState::LoanActive(LoanActiveState {
                total_amount: 0,
                borrowed_amount: 0,
                started_at: 0,
                expired_at: 0,
                lender: Pubkey::new(&[0u8; 32]),
            }),
        };

        let acc_size = 8 + largest_instance.try_to_vec().unwrap().len();
        acc_size
    }
}
