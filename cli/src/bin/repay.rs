use anchor_client::Client;
use anyhow::Result;
use cli::{get_cluster, load_program_from_idl, Keypair};
use solana_clap_utils::input_parsers::keypair_of;
use solana_clap_utils::input_parsers::pubkey_of;
use solana_sdk::{pubkey::Pubkey, signature::Signer, sysvar};
use spl_associated_token_account::get_associated_token_address;
use structopt::StructOpt;
use liqz::{NFTDeposit, NFTPool};

#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,

    #[structopt(long, env)]
    pool_owner_address: String,

    #[structopt(long, env)]
    borrower_wallet_keypair: String,

    #[structopt(long, env)]
    lender_wallet_address: String,

    #[structopt(long, env)]
    dai_mint_address: Pubkey,

    #[structopt(long, env)]
    nft_mint_address: Pubkey,

    #[structopt(long, env)]
    deposit_id: Pubkey,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();
    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(load_program_from_idl);

    let borrower_wallet_keypair =
        keypair_of(&Opt::clap().get_matches(), "borrower-wallet-keypair").unwrap();
    let pool_owner_address = pubkey_of(&Opt::clap().get_matches(), "pool-owner-address").unwrap();
    let lender_wallet_address =
        pubkey_of(&Opt::clap().get_matches(), "lender-wallet-address").unwrap();

    let client = Client::new(get_cluster(), Keypair::copy(&borrower_wallet_keypair));
    let program = client.program(program_id);

    let pool = NFTPool::get_address(&program.id());

    let tx = program
        .request()
        .accounts(liqz::accounts::AccountsRepay {
            pool,
            borrower_wallet_account: borrower_wallet_keypair.pubkey(),

            pool_owner_dai_account: get_associated_token_address(
                &pool_owner_address,
                &opt.dai_mint_address,
            ),
            borrower_dai_account: get_associated_token_address(
                &borrower_wallet_keypair.pubkey(),
                &opt.dai_mint_address,
            ),
            lender_dai_account: get_associated_token_address(
                &lender_wallet_address,
                &opt.dai_mint_address,
            ),

            borrower_nft_account: get_associated_token_address(
                &borrower_wallet_keypair.pubkey(),
                &opt.nft_mint_address,
            ),
            pool_nft_account: get_associated_token_address(&pool, &opt.nft_mint_address),

            deposit_account: NFTDeposit::get_address(
                &program_id,
                &opt.nft_mint_address,
                &borrower_wallet_keypair.pubkey(),
                &opt.deposit_id,
            ),

            spl_program: spl_token::id(),
            clock: sysvar::clock::id(),
        })
        .args(liqz::instruction::Repay {})
        .signer(&borrower_wallet_keypair)
        .send()?;

    println!("The transaction is {}", tx);

    Ok(())
}
