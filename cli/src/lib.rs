use anchor_client::Cluster;
use derive_more::Deref;
use serde::Deserialize;
use serde_json::from_reader;
use solana_sdk::pubkey::Pubkey;
use std::{fs::File, str::FromStr};

#[derive(Debug, Deref)]
pub struct Keypair(#[deref] pub solana_sdk::signature::Keypair, String);

impl Keypair {
    pub fn copy(key: &solana_sdk::signature::Keypair) -> solana_sdk::signature::Keypair {
        solana_sdk::signature::Keypair::from_bytes(&key.to_bytes()).unwrap()
    }
}

impl FromStr for Keypair {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            solana_sdk::signature::Keypair::from_base58_string(s),
            s.into(),
        ))
    }
}

impl Clone for Keypair {
    fn clone(&self) -> Self {
        self.1.parse().unwrap()
    }
}
#[derive(Deserialize)]
struct IDL {
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    address: String,
}

pub fn load_program_from_idl() -> Pubkey {
    let f = File::open("target/idl/liqz.json").unwrap();
    let m: IDL = from_reader(f).unwrap();
    m.metadata.address.parse().unwrap()
}

pub fn get_cluster() -> Cluster {
    Cluster::Custom(
        "https://api.devnet.solana.com".into(),
        "wss://api.devnet.solana.com".into(),
    )
}
