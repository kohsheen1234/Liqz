cli prog +ARGS="":
  cargo run --release --bin {{prog}} -- {{ARGS}}

build:
  anchor build
  
deploy: build
  anchor deploy

token +ARGS="":
  spl-token {{ARGS}}

test:
  anchor test --skip-deploy

b58keypair:
  #!/usr/bin/env python3
  import base58
  import json
  from pathlib import Path

  with open(Path.home() / ".config/solana/id.json") as f:
      j = json.load(f)
  print(base58.b58encode(bytes(j)))

transfer-to-pool dst:
  spl-token transfer $liz_MINT_ADDRESS 10000 {{dst}}
  spl-token transfer $TAI_MINT_ADDRESS 10000 {{dst}}


combo:
  just deploy

  cargo build --bins --release
  
  just cli initialize
  