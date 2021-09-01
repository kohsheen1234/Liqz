use anchor_lang::prelude::Pubkey;

use crate::{utils, DerivedAccountIdentifier, NFTBid, liqzError};
use anchor_lang::prelude::*;
use fehler::{throw, throws};

impl DerivedAccountIdentifier for NFTBid {
    const SEED: &'static [u8] = b"liqzNFTBid";
}

impl NFTBid {
    #[throws(ProgramError)]
    pub fn ensure<'info>(
        program_id: &Pubkey,
        nft_mint: &Pubkey,
        wallet: &AccountInfo<'info>,
        bid_account: &AccountInfo<'info>,
        rent: &Sysvar<'info, Rent>,
        system: &AccountInfo<'info>,
    ) -> ProgramAccount<'info, Self> {
        let (_, bump) = Self::get_address_with_bump(program_id, nft_mint, wallet.key);

        Self::verify_address(program_id, nft_mint, wallet.key, bump, bid_account.key)?;

        if !crate::utils::is_account_allocated(bid_account) {
            let instance = NFTBid { price: 0, qty: 0 };

            let acc_size = 8 + instance
                .try_to_vec()
                .map_err(|_| ProgramError::Custom(1))?
                .len() as u64;

            let seeds_with_bump: &[&[_]] = &[
                Self::SEED,
                &nft_mint.to_bytes(),
                &wallet.key.to_bytes(),
                &[bump],
            ];

            utils::create_derived_account_with_seed(
                program_id,
                wallet,
                seeds_with_bump,
                bid_account,
                acc_size,
                &rent,
                &system,
            )?;

            {
                let mut data = bid_account.try_borrow_mut_data()?;
                let mut cursor = std::io::Cursor::new(&mut **data);
                instance.try_serialize(&mut cursor)?;
            }
        }

        ProgramAccount::try_from(bid_account)?
    }

    #[throws(liqzError)]
    pub fn trade(&mut self, qty: u64) {
        if qty > self.qty {
            throw!(liqzError::NFTOvertrade)
        }
        self.qty -= qty;
        if self.qty == 0 {
            self.price = 0;
        }
    }

    pub fn set(&mut self, price: u64, qty: u64) {
        self.price = price;
        self.qty = qty;
    }

    pub fn cancel(&mut self) {
        self.price = 0;
        self.qty = 0;
    }

    // An program derived account that stores nft bid
    // The address of the account is computed as follow:
    // address = find_program_address([NFTBid::SEED, nft_mint_address, user_wallet_address], program_id)
    // only the liqz_contract_address can change the data in this account
    pub fn get_address(program_id: &Pubkey, nft_mint: &Pubkey, wallet: &Pubkey) -> Pubkey {
        Self::get_address_with_bump(program_id, nft_mint, wallet).0
    }

    pub(crate) fn get_address_with_bump(
        program_id: &Pubkey,
        nft_mint: &Pubkey,
        wallet: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[Self::SEED, &nft_mint.to_bytes(), &wallet.to_bytes()],
            program_id,
        )
    }

    #[throws(ProgramError)]
    pub fn verify_address(
        program_id: &Pubkey,
        nft_mint: &Pubkey,
        wallet: &Pubkey,
        bump: u8,
        address: &Pubkey,
    ) {
        let addr = Pubkey::create_program_address(
            &[
                Self::SEED,
                &nft_mint.to_bytes(),
                &wallet.to_bytes(),
                &[bump],
            ],
            program_id,
        )?;

        if &addr != address {
            throw!(liqzError::NFTBidAddressNotCorrect);
        }
    }
}
