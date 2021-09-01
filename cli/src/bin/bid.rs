use anchor_client::Client;
use anyhow::Result;
use cli::{get_cluster, load_program_from_idl, Keypair};
use solana_clap_utils::input_parsers::keypair_of;
use solana_sdk::{pubkey::Pubkey, signature::Signer, system_program, sysvar};
use spl_associated_token_account::get_associated_token_address;
use structopt::StructOpt;
use liqz::{NFTBid, NFTPool};

#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,

    #[structopt(long, env)]
    lender_wallet_keypair: String,

    #[structopt(long, env)]
    dai_mint_address: Pubkey,

    #[structopt(long, env)]
    nft_mint_address: Pubkey,

    #[structopt(long)]
    price: f64,

    #[structopt(long)]
    qty: u64,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();
    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(load_program_from_idl);

    let lender_wallet_keypair =
        keypair_of(&Opt::clap().get_matches(), "lender-wallet-keypair").unwrap();

    let client = Client::new(get_cluster(), Keypair::copy(&lender_wallet_keypair));
    let program = client.program(program_id);

    let pool = NFTPool::get_address(&program.id());

    let tx = program
        .request()
        .accounts(liqz::accounts::AccountsPlaceBid {
            pool,
            lender_wallet_account: lender_wallet_keypair.pubkey(),

            nft_mint: opt.nft_mint_address,
            lender_dai_account: get_associated_token_address(
                &lender_wallet_keypair.pubkey(),
                &opt.dai_mint_address,
            ),

            bid_account: NFTBid::get_address(
                &program_id,
                &opt.nft_mint_address,
                &lender_wallet_keypair.pubkey(),
            ),

            spl_program: spl_token::id(),
            system_program: system_program::id(),
            rent: sysvar::rent::id(),
        })
        .args(liqz::instruction::PlaceBid {
            price: (opt.price * 10f64.powf(9.)) as u64,
            qty: opt.qty,
        })
        .signer(&lender_wallet_keypair)
        .send()?;

    println!("The transaction is {}", tx);

    Ok(())
}
