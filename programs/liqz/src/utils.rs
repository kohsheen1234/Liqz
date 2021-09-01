use anchor_lang::prelude::*;
use anchor_spl::token::Mint;
use fehler::throws;
use solana_program::{instruction::Instruction, program::invoke_signed, system_program};
use solana_program::{program::invoke, system_instruction};

#[throws(ProgramError)]
pub fn create_derived_account_with_seed<'info>(
    program_id: &Pubkey, // The program ID of liqz Contract
    funder: &AccountInfo<'info>,
    seeds_with_bump: &[&[u8]],
    account: &AccountInfo<'info>,
    acc_size: u64,
    rent: &Sysvar<'info, Rent>,
    system: &AccountInfo<'info>,
) {
    let required_lamports = rent.minimum_balance(acc_size as usize).max(1);

    invoke_signed(
        &system_instruction::create_account(
            funder.key,
            account.key,
            required_lamports,
            acc_size,
            program_id,
        ),
        &[funder.clone(), account.clone(), system.clone()],
        &[seeds_with_bump],
    )?;
}

#[throws(ProgramError)]
pub fn create_associated_token_account<'info>(
    wallet: &AccountInfo<'info>,
    funder: &AccountInfo<'info>,
    mint: &CpiAccount<'info, Mint>,
    token_account: &AccountInfo<'info>,
    ata_program: &AccountInfo<'info>,
    spl_program: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &Sysvar<'info, Rent>,
) {
    // Accounts expected by this instruction:
    //
    //   0. `[writeable,signer]` Funding account (must be a system account)
    //   1. `[writeable]` Associated token account address to be created
    //   2. `[]` Wallet address for the new associated token account
    //   3. `[]` The token mint for the new associated token account
    //   4. `[]` System program
    //   5. `[]` SPL Token program
    //   6. `[]` Rent sysvar
    let ix = Instruction {
        program_id: spl_associated_token_account::id(),
        accounts: vec![
            AccountMeta::new(*funder.key, true),
            AccountMeta::new(*token_account.key, false),
            AccountMeta::new_readonly(*wallet.key, false),
            AccountMeta::new_readonly(*mint.to_account_info().key, false),
            AccountMeta::new_readonly(*system_program.key, false),
            AccountMeta::new_readonly(*spl_program.key, false),
            AccountMeta::new_readonly(*rent.to_account_info().key, false),
        ],
        data: vec![],
    };

    invoke(
        &ix,
        &[
            funder.clone(),
            token_account.clone(),
            wallet.clone(),
            mint.to_account_info(),
            ata_program.clone(),
            system_program.clone(),
            spl_program.clone(),
            rent.to_account_info(),
        ],
    )?;
}

pub fn is_account_allocated(acc: &AccountInfo) -> bool {
    //    if the account has non zero lamports or has data stored or has the owner != system_program, then this account is already allocated
    acc.lamports() != 0 || !acc.data_is_empty() || !system_program::check_id(&acc.owner)
}
