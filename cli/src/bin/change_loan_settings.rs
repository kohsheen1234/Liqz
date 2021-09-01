use anchor_client::Client;
use anyhow::Result;
use cli::{get_cluster, Keypair};
use solana_clap_utils::input_parsers::keypair_of;
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use structopt::StructOpt;
use liqz::NFTPool;

#[derive(Debug, StructOpt)]
#[structopt(name = "transact", about = "Making transactions to the liqz Protocol")]
struct Opt {
    #[structopt(long, env, short = "p")]
    liqz_program_address: Option<Pubkey>,

    #[structopt(long, env)]
    pool_owner_keypair: String,

    #[structopt(long, env)]
    incentive: Option<u64>,

    #[structopt(long, env)]
    interest_rate: Option<u64>,

    #[structopt(long, env)]
    service_fee_rate: Option<u64>,

    #[structopt(long, env)]
    max_loan_duration: Option<i64>,

    #[structopt(long, env)]
    mortgage_rate: Option<u64>,
}

fn main() -> Result<()> {
    solana_logger::setup_with("solana=debug");

    let opt = Opt::from_args();
    let program_id = opt
        .liqz_program_address
        .unwrap_or_else(cli::load_program_from_idl);
    println!("program_id: {}", program_id);

    let pool_owner_keypair = keypair_of(&Opt::clap().get_matches(), "pool-owner-keypair").unwrap();

    let client = Client::new(get_cluster(), Keypair::copy(&pool_owner_keypair));
    let program = client.program(program_id);

    let pool = NFTPool::get_address(&program.id());

    let tx = program
        .request()
        .accounts(liqz::accounts::AccountsChangeLoanSetting {
            owner: pool_owner_keypair.pubkey(),
            pool,
        })
        .args(liqz::instruction::ChangeLoanSettings {
            incentive: opt.incentive,
            interest_rate: opt.interest_rate,
            service_fee_rate: opt.service_fee_rate,
            max_loan_duration: opt.max_loan_duration,
            mortgage_rate: opt.mortgage_rate,
        })
        .signer(&pool_owner_keypair)
        .send()?;

    println!("The transaction is {}", tx);
    println!("Pool address: {}", pool);

    Ok(())
}
