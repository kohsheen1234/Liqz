use anchor_client::Client;
use anyhow::Result;
use cli::get_cluster;
use solana_clap_utils::input_parsers::keypair_of;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use structopt::StructOpt;
use liqz::NFTBid;
#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,

    #[structopt(long, env)]
    nft_mint_address: Pubkey,

    #[structopt(long, env)]
    lender_wallet_keypair: String,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();

    let lender_wallet_keypair =
        keypair_of(&Opt::clap().get_matches(), "lender-wallet-keypair").unwrap();

    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(cli::load_program_from_idl);

    let bid_account = NFTBid::get_address(
        &program_id,
        &opt.nft_mint_address,
        &lender_wallet_keypair.pubkey(),
    );

    let client = Client::new(get_cluster(), lender_wallet_keypair);
    let program = client.program(program_id);

    let content: NFTBid = program.account(bid_account)?;

    println!(
        "The bid address is {} with content {:?}",
        bid_account, content
    );

    Ok(())
}
