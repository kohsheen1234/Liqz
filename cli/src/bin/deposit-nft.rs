use anchor_client::Client;
use anyhow::Result;
use cli::{get_cluster, load_program_from_idl, Keypair};
use rand::rngs::OsRng;
use solana_clap_utils::input_parsers::keypair_of;
use solana_sdk::{pubkey::Pubkey, signature::Signer, system_program, sysvar};
use spl_associated_token_account::get_associated_token_address;
use structopt::StructOpt;
use liqz::{NFTDeposit, NFTPool};

#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,

    #[structopt(long, env)]
    borrower_wallet_keypair: String,

    #[structopt(long, env)]
    liz_mint_address: Pubkey,

    #[structopt(long, env)]
    nft_mint_address: Pubkey,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();
    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(load_program_from_idl);

    let borrower_wallet_keypair =
        keypair_of(&Opt::clap().get_matches(), "borrower-wallet-keypair").unwrap();

    let client = Client::new(get_cluster(), Keypair::copy(&borrower_wallet_keypair));
    let program = client.program(program_id);

    let pool = NFTPool::get_address(&program.id());

    let deposit_id = solana_sdk::signature::Keypair::generate(&mut OsRng).pubkey();

    let tx = program
        .request()
        .accounts(liqz::accounts::AccountsDepositNFT {
            pool,
            borrower_wallet_account: borrower_wallet_keypair.pubkey(),

            nft_mint: opt.nft_mint_address,
            liz_mint: opt.liz_mint_address,

            borrower_nft_account: get_associated_token_address(
                &borrower_wallet_keypair.pubkey(),
                &opt.nft_mint_address
            ),
            pool_nft_account: get_associated_token_address(&pool, &opt.nft_mint_address),

            pool_liz_account: get_associated_token_address(&pool, &opt.liz_mint_address),
            borrower_liz_account: get_associated_token_address(
                &borrower_wallet_keypair.pubkey(),
                &opt.liz_mint_address,
            ),

            deposit_account: NFTDeposit::get_address(
                &program_id,
                &opt.nft_mint_address,
                &borrower_wallet_keypair.pubkey(),
                &deposit_id,
            ),

            ata_program: spl_associated_token_account::id(),
            spl_program: spl_token::id(),
            rent: sysvar::rent::id(),
            system_program: system_program::id(),
        })
        .args(liqz::instruction::DepositNft { deposit_id })
        .signer(&borrower_wallet_keypair)
        .send()?;

    println!("The transaction is {}", tx);
    println!("The deposit_id is {}", deposit_id);

    Ok(())
}
