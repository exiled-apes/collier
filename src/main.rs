use borsh::de::BorshDeserialize;
use gumdrop::Options;
use metaplex_token_metadata::{
    instruction::update_metadata_accounts,
    state::{Metadata, Data, Creator},
};
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::json;
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
    rpc_request::RpcRequest,
    rpc_response::Response,
};
use solana_sdk::{
    account::ReadableAccount, program_pack::Pack, pubkey::Pubkey, signature::read_keypair_file,
    signer::Signer, transaction::Transaction,
};
use spl_token::state::Account;
use std::{error::Error, time::Duration};

#[derive(Clone, Debug, Options)]
struct Args {
    #[options(help = "slite db path", default_expr = "default_db_path()")]
    db: String,
    #[options(help = "rpc server", default_expr = "default_rpc_url()", meta = "r")]
    rpc: String,
    #[options(command)]
    command: Option<Command>,
}

fn default_db_path() -> String {
    "collier.db".to_owned()
}

fn default_rpc_url() -> String {
    "https://api.mainnet-beta.solana.com".to_owned()
}

#[derive(Clone, Debug, Options)]
enum Command {
    MineHolders(MineHolders),
    MineMetadata(MineMetadata),
    ListMetadataUris(ListMetadataUris),
    // MineTransactions(MineTransactions),
    RescueSlatts(RescueSlatts),
}

#[derive(Clone, Debug, Options)]
struct MineMetadata {
    #[options(help = "creator address")]
    creator_address: String,
}

#[derive(Clone, Debug, Options)]
struct ListMetadataUris {
    #[options(help = "creator address")]
    creator_address: String,
}

#[derive(Clone, Debug, Options)]
struct MineHolders {
    #[options(help = "creator address")]
    creator_address: String,
}

#[derive(Clone, Debug, Options)]
struct RescueSlatts {
    #[options(help = "update authority keypair")]
    update_authority: String,
}

// #[derive(Clone, Debug, Options)]
// struct MineTransactions {
//     #[options(help = "account id")]
//     account_id: String,
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse_args_default_or_exit();
    match args.clone().command {
        None => todo!(),
        Some(command) => match command {
            Command::ListMetadataUris(opts) => list_metadata_uris(args, opts).await,
            Command::MineHolders(opts) => mine_holders(args, opts).await,
            Command::MineMetadata(opts) => mine_metadata(args, opts).await,
            Command::RescueSlatts(opts) => rescue_slatts(args, opts).await,
        },
    }
}

async fn list_metadata_uris(args: Args, _opts: ListMetadataUris) -> Result<(), Box<dyn Error>> {
    let db = Connection::open(args.db)?;

    let timeout = Duration::from_secs(500); // TODO read from Args?
    let rpc = RpcClient::new_with_timeout(args.rpc, timeout);

    let mut stmt = db.prepare("SELECT metadata_address FROM metadata")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let metadata_address: String = row.get(0)?;

        let mut tries = 0;
        let account = loop {
            tries += 1;
            match rpc.get_account(&metadata_address.parse()?) {
                Ok(account) => break Some(account),
                Err(err) => {
                    eprint!("!");
                    if tries > 5 {
                        eprintln!("{} {}", metadata_address, err);
                        break None;
                    }
                }
            }
        };

        if let Some(account) = account {
            let metadata = Metadata::deserialize(&mut account.data())?;
            let uri = metadata.data.uri.trim_matches(char::from(0));
            println!("{},{}", metadata_address, uri);
        }
    }

    Ok(())
}

async fn mine_holders(args: Args, _opts: MineHolders) -> Result<(), Box<dyn Error>> {
    let timeout = Duration::from_secs(500); // TODO read from Args?
    let rpc = RpcClient::new_with_timeout(args.rpc, timeout);

    let db = Connection::open(args.db)?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS holders (
            mint_address   text primary key,
            holder_address text
        )",
        params![],
    )?;

    let mut stmt = db.prepare("SELECT mint_address FROM metadata")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let mint_address: String = row.get(0)?;
        let mint_address = mint_address.parse()?;

        let token_accounts = get_token_largest_accounts(&rpc, mint_address)?;
        let token_accounts = token_accounts.value;
        for token_account in token_accounts {
            if token_account.amount == "1" {
                let account = rpc.get_account(&token_account.address.parse()?)?;
                let account = Account::unpack(&mut account.data())?;
                db.execute(
                    "DELETE FROM holders WHERE mint_address = ?1",
                    params![mint_address.to_string()],
                )?;
                db.execute(
                    "INSERT INTO holders (mint_address, holder_address) VALUES (?1, ?2)",
                    params![mint_address.to_string(), account.owner.to_string()],
                )?;
            }
        }
    }

    Ok(())
}

async fn mine_metadata(args: Args, opts: MineMetadata) -> Result<(), Box<dyn Error>> {
    let db = Connection::open(args.db)?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS creators (
            creator_address  text,
            metadata_address text,
            UNIQUE(creator_address, metadata_address)
        )",
        params![],
    )?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            metadata_address text primary key,
            mint_address     text unique
        )",
        params![],
    )?;

    let timeout = Duration::from_secs(500); // TODO read from Args?
    let rpc = RpcClient::new_with_timeout(args.rpc, timeout);

    let metadata_accounts = rpc.get_program_accounts_with_config(
        &metaplex_token_metadata::id(),
        RpcProgramAccountsConfig {
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                ..RpcAccountInfoConfig::default()
            },
            filters: Some(vec![RpcFilterType::Memcmp(Memcmp {
                offset: 1 + // key,
                       32 + // update auth
                       32 + // mint
                        4 + // name string length
                       32 + // max name length
                        4 + // uri string length
                      200 + // max uri length
                        4 + // symbol string length
                       10 + // max symbol length
                        2 + // seller fee basis points
                        1 + // whether or not there is a creators vec
                        4, // creators vec length
                // bytes: MemcmpEncodedBytes::Binary(opts.creator_address.to_string()),
                bytes: MemcmpEncodedBytes::Base58(opts.creator_address.to_string()),
                encoding: None,
            })]),
            ..RpcProgramAccountsConfig::default()
        },
    )?;

    for (metadata_address, metadata) in metadata_accounts {
        let metadata = Metadata::deserialize(&mut metadata.data())?;
        db.execute(
            "INSERT OR REPLACE INTO metadata (metadata_address, mint_address) VALUES (?1, ?2)",
            params![metadata_address.to_string(), metadata.mint.to_string()],
        )?;
        db.execute(
            "INSERT OR REPLACE INTO creators (creator_address, metadata_address) VALUES (?1, ?2)",
            params![opts.creator_address, metadata_address.to_string()],
        )?;
    }

    Ok(())
}

async fn rescue_slatts(args: Args, opts: RescueSlatts) -> Result<(), Box<dyn Error>> {
    let timeout = Duration::from_secs(500); // TODO read from Args?
    let rpc = RpcClient::new_with_timeout(args.rpc, timeout);

    let db = Connection::open(args.db)?;
    let mut stmt = db.prepare("SELECT metadata_address, mint_address FROM metadata")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let metadata_address: String = row.get(0)?;
        let metadata_address = metadata_address.parse()?;

        // let mint_address: String = row.get(1)?;

        let mut tries = 0;
        let account = loop {
            tries += 1;
            match rpc.get_account(&metadata_address) {
                Ok(account) => break Some(account),
                Err(err) => {
                    eprint!("!");
                    if tries > 5 {
                        eprintln!("{} {}", metadata_address, err);
                        break None;
                    }
                }
            }
        };

        if let Some(account) = account {
            let recent_blockhash = rpc.get_latest_blockhash()?;

            let metadata = Metadata::deserialize(&mut account.data())?;
            let data = metadata.data;

            let creators = data.clone().creators.unwrap();
            if creators.len() != 4 {
                continue;
            }

            let update_authority = read_keypair_file(opts.update_authority.clone())?;

            let creators: Option<Vec<Creator>> = Some(vec![
                creators[0].clone(),
                Creator {
                    address: update_authority.pubkey(),
                    verified: true,
                    share: 100,
                },
            ]);

            let instruction = update_metadata_accounts(
                metaplex_token_metadata::id(),
                metadata_address,
                update_authority.pubkey(),
                None,
                Some(Data {
                    name: data.name,
                    symbol: data.symbol,
                    uri: data.uri,
                    seller_fee_basis_points: data.seller_fee_basis_points,
                    creators,
                }),
                None,
            );

            let instructions = [instruction];

            let signing_keypairs = [&update_authority];

            let tx = Transaction::new_signed_with_payer(
                &instructions,
                Some(&update_authority.pubkey()),
                &signing_keypairs,
                recent_blockhash,
            );

            // eprint!("{} {} {} > ", update_authority.pubkey(), metadata_address, mint_address);
            // eprintln!("{:?}", tx);

            let res = rpc.simulate_transaction(&tx)?;
            eprintln!("{} {:?}\n", metadata_address, res);
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct RpcTokenAccounts {
    pub address: String,
    pub amount: String,
    pub decimals: u8,
}

fn get_token_largest_accounts(
    rpc: &RpcClient,
    mint_address: Pubkey,
) -> Result<Response<Vec<RpcTokenAccounts>>, Box<dyn Error>> {
    let method = "getTokenLargestAccounts";
    let request = RpcRequest::Custom { method };
    let params = json!([mint_address.to_string()]);
    Ok(rpc.send(request, params)?)
}
