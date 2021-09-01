use anchor_client::Client;
use anyhow::Result;
use cli::get_cluster;
use rand::rngs::OsRng;
use solana_clap_utils::input_parsers::pubkey_of;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use structopt::StructOpt;
use liqz::NFTDeposit;

#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,

    #[structopt(long, env)]
    nft_mint_address: Pubkey,

    #[structopt(long, env)]
    borrower_wallet_address: String,

    #[structopt(long, env)]
    deposit_id: Pubkey,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();
    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(cli::load_program_from_idl);

    let borrower_wallet_address =
        pubkey_of(&Opt::clap().get_matches(), "borrower-wallet-address").unwrap();

    let deposit_account = NFTDeposit::get_address(
        &program_id,
        &opt.nft_mint_address,
        &borrower_wallet_address,
        &opt.deposit_id,
    );

    let client = Client::new(get_cluster(), Keypair::generate(&mut OsRng));
    let program = client.program(program_id);

    let content: NFTDeposit = program.account(deposit_account)?;

    println!("Account: {:?}, Deposit: {:?}", deposit_account, content);

    Ok(())
}
