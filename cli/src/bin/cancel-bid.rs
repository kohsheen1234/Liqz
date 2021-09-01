use anchor_client::Client;
use anyhow::Result;
use cli::{get_cluster, load_program_from_idl, Keypair};
use solana_clap_utils::input_parsers::keypair_of;
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use spl_associated_token_account::get_associated_token_address;
use structopt::StructOpt;
use liqz::NFTBid;

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

    let tx = program
        .request()
        .accounts(liqz::accounts::AccountsCancelBid {
            lender_wallet_account: lender_wallet_keypair.pubkey(),

            nft_mint: opt.nft_mint_address,
            lender_dai_account: dbg!(get_associated_token_address(
                &lender_wallet_keypair.pubkey(),
                &opt.dai_mint_address
            )),

            bid_account: dbg!(NFTBid::get_address(
                &program_id,
                &opt.nft_mint_address,
                &lender_wallet_keypair.pubkey(),
            )),

            spl_program: spl_token::id(),
        })
        .args(liqz::instruction::CancelBid { revoke: true })
        .signer(&lender_wallet_keypair)
        .send()?;

    println!("The transaction is {}", tx);

    Ok(())
}
