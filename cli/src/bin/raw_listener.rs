use anyhow::{Error, Result};
use fehler::throws;
use log::info;
use solana_client::pubsub_client::PubsubClient;
use solana_client::rpc_config::RpcTransactionLogsConfig;
use solana_client::rpc_config::RpcTransactionLogsFilter;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    #[structopt(long, env, default_value = "https://devnet.solana.com")]
    solana_rpc_url: String,

    #[structopt(long, env, default_value = "wss://devnet.solana.com")]
    solana_sub_url: String,

    #[structopt(long, short = "p", env)]
    liqz_program_addr: String,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let _ = env_logger::init();

    let program_id = opt.liqz_program_addr;
    info!("Listening to {}", program_id);
    loop {
        match imp(&opt.solana_sub_url, &program_id.to_string()) {
            Ok(_) => unreachable!(),
            Err(_) => {
                // error!("{}", e)
            }
        }
    }
}

#[allow(unreachable_code)]
#[throws(Error)]
fn imp(solana_sub_url: &str, program_id: &str) {
    let (_pc, rx) = PubsubClient::logs_subscribe(
        solana_sub_url,
        RpcTransactionLogsFilter::Mentions(vec![program_id.into()]),
        RpcTransactionLogsConfig { commitment: None },
    )?;

    loop {
        let msg = rx.recv()?;
        for l in msg.value.logs {
            info!("{}", l);
        }
    }
}
