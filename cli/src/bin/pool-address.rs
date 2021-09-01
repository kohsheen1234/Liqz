use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use structopt::StructOpt;
use liqz::NFTPool;

#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();
    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(cli::load_program_from_idl);

    let pool = NFTPool::get_address(&program_id);

    println!("The pool address is {}", pool);

    Ok(())
}
