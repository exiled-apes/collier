use borsh::de::BorshDeserialize;
use gumdrop::Options;
use metaplex_token_metadata::state::Metadata;
use rusqlite::{params, Connection};
use solana_account_decoder::UiAccountEncoding;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use solana_sdk::account::ReadableAccount;
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
    MineTransactions(MineTransactions),
}

#[derive(Clone, Debug, Options)]
struct MineMetadata {
    #[options(help = "creator address")]
    creator_address: String,
}

#[derive(Clone, Debug, Options)]
struct MineHolders {
    #[options(help = "creator address")]
    creator_address: String,
}

#[derive(Clone, Debug, Options)]
struct MineTransactions {
    #[options(help = "account id")]
    account_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse_args_default_or_exit();
    match args.clone().command {
        None => todo!(),
        Some(command) => match command {
            Command::MineHolders(opts) => mine_holders(args, opts).await,
            Command::MineMetadata(opts) => mine_metadata(args, opts).await,
            Command::MineTransactions(opts) => mine_transactions(args, opts).await,
        },
    }
}

async fn mine_holders(_args: Args, _opts: MineHolders) -> Result<(), Box<dyn Error>> {
    Ok(())
}

async fn mine_metadata(args: Args, opts: MineMetadata) -> Result<(), Box<dyn Error>> {
    let db = Connection::open(args.db)?;
    db.execute(
        "CREATE TABLE IF NOT EXISTS metadata (
            metadata_address text primary key,
            mint_address     text unique
        )",
        params![],
    )?;

    let timeout = Duration::from_secs(500);
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
                bytes: MemcmpEncodedBytes::Binary(opts.creator_address.to_string()),
                encoding: None,
            })]),
            ..RpcProgramAccountsConfig::default()
        },
    )?;

    for (metadata_address, metadata) in metadata_accounts {
        if {
            let count: Result<u8, rusqlite::Error> = db.query_row(
                "SELECT COUNT(*) FROM metadata WHERE metadata_address like ?1",
                params![opts.creator_address.to_string()],
                |row| row.get(0),
            );
            count? > 0
        } {
            continue;
        }

        let metadata = Metadata::deserialize(&mut metadata.data())?;
        db.execute(
            "INSERT INTO metadata (
                metadata_address,
                mint_address
            ) VALUES (
                ?1,
                ?2
            )",
            params![metadata_address.to_string(), metadata.mint.to_string()],
        )?;
    }

    Ok(())
}

async fn mine_transactions(args: Args, opts: MineTransactions) -> Result<(), Box<dyn Error>> {
    let rpc = RpcClient::new(args.rpc);

    let account_id = opts.account_id.parse()?;
    let account = rpc.get_account(&account_id)?;
    let _deleteme = account;

    let _db = Connection::open(args.db)?;

    Ok(())
}
