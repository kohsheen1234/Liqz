// client.js is used to introduce the reader to generating clients from IDLs.
// It is not expected users directly test with this example. For a more
// ergonomic example, see `tests/basic-0.js` in this workspace.

const anchor = require('@project-serum/anchor');
const solanaWeb3 = require('@solana/web3.js');
const splToken = require('@solana/spl-token');
const bs58 = require('bs58');


// Configure the client to use the local cluster.
const provider = anchor.Provider.env()
anchor.setProvider(provider);


// const idl = JSON.parse(require('fs').readFileSync('./target/idl/liqz.json', 'utf8'));
// const programId = new anchor.web3.PublicKey('91aE2UGTmGfy9FVCPB9PFoNbEokDoPBKh8nitW4QPwxp');
// const program = new anchor.Program(idl, programId);

const program = anchor.workspace.liqz;

const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID = new anchor.web3.PublicKey(
    'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL',
);
async function findAssociatedTokenAddress(
    walletAddress,
    tokenMintAddress
) {
    return (await solanaWeb3.PublicKey.findProgramAddress(
        [
            walletAddress.toBuffer(),
            splToken.TOKEN_PROGRAM_ID.toBuffer(),
            tokenMintAddress.toBuffer(),
        ],
        SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID
    ))[0];
}

async function main() {
    program.addEventListener("CalledInitialize", (event, slot) => { console.log("Event:", event) });

    let authority = provider.wallet.payer;


    const seed = await anchor.web3.Keypair.generate();

    const [contract_acc, _nonce] = await anchor.web3.PublicKey.findProgramAddress(
        [authority.publicKey.toBuffer()],
        program.programId
    );
    const liz_mint = new anchor.web3.PublicKey(process.env.liz_MINT_ADDRESS);
    const liz_token_address = await findAssociatedTokenAddress(contract_acc, liz_mint);
    const tai_mint = new anchor.web3.PublicKey(process.env.TAI_MINT_ADDRESS);
    const dai_mint = new anchor.web3.PublicKey(process.env.DAI_MINT_ADDRESS);
    console.log(authority)
    console.log(contract_acc)

    const tx = await program.rpc.initialize(
        seed.publicKey.toBuffer(),
        {
            accounts: {
                contractAccount: contract_acc,
                authority: authority.publicKey,
                lizMint: liz_mint,
                lizToken: liz_token_address,
                taiMint: tai_mint,
                taiToken: await findAssociatedTokenAddress(contract_acc, tai_mint),
                daiMint: dai_mint,
                daiToken: await findAssociatedTokenAddress(contract_acc, dai_mint),
                splProgram: splToken.TOKEN_PROGRAM_ID,
                rent: anchor.web3.SYSVAR_RENT_PUBKEY,
                system: anchor.web3.SystemProgram.programId,
            },
            signers: [authority, contract_acc]
        });

    console.log("Your transaction signature", tx);
}

console.log('Running client.');
main().then(() => console.log('Success'));

