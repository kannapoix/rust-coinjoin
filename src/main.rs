use actix_web::{web, App, HttpServer};
use serde::{Deserialize, Serialize};

mod handlers;
mod utils;

#[derive(Serialize, Deserialize, Debug)]
struct CoinJoinInput {
    outpoint: String,
    psbt_input: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(
            web::scope("/api").service(
                web::scope("/v1")
                    .service(handlers::record_input)
                    .service(handlers::generate_psbt),
            ),
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use bdk::bitcoin::consensus::encode::serialize_hex;
    use bdk::bitcoin::Network;
    use bdk::blockchain::{noop_progress, ElectrumBlockchain};
    use bdk::database::MemoryDatabase;
    use bdk::keys::bip39::Mnemonic;
    use bdk::keys::{DerivableKey, ExtendedKey};
    use bdk::wallet::{AddressIndex, Wallet};

    type InnerWallet = Wallet<ElectrumBlockchain, MemoryDatabase>;
    type PSBT = bdk::bitcoin::util::psbt::PartiallySignedTransaction;

    const MNEMONIC_DIR: &str = "./data/client/mnemonic";
    const SERVER_INPUT_DIR: &str = "./data/client/server_inputs";
    const OUTPUT_DIR: &str = "./data/client/outputs";
    const PSBT_PATH: &str = "./data/psbt.txt";

    fn list_signed_txs(mut psbt: PSBT, wallets: &Vec<InnerWallet>) -> Vec<PSBT> {
        let mut psbts = Vec::new();
        for wallet in wallets.iter() {
            wallet.sync(noop_progress(), None);
            let mut psbt_for_each_wallet = psbt.clone();
            match wallet.sign(&mut psbt_for_each_wallet, bdk::SignOptions::default()) {
                Ok(result) => {
                    println!("Sign status is {}", result);
                }
                Err(error) => {
                    println!("My Error {:?}", error)
                }
            }
            psbts.push(psbt_for_each_wallet.clone())
        }
        psbts
    }

    fn merge_psbts(mut psbts: Vec<PSBT>) -> Vec<PSBT> {
        return if psbts.len() == 1 {
            psbts
        } else {
            let mut merged_psbt = psbts.pop().unwrap();
            merged_psbt.merge(psbts.pop().unwrap()).unwrap();
            psbts.push(merged_psbt);
            merge_psbts(psbts)
        };
    }

    fn setup_client_wallets() -> Vec<Wallet<ElectrumBlockchain, MemoryDatabase>> {
        let mut mnemonics: Vec<Mnemonic> = Vec::new();
        for file_name in Path::new(MNEMONIC_DIR)
            .read_dir()
            .expect("read_dir call failed")
        {
            if let Ok(file_name) = file_name {
                let file_path = file_name.path();
                match fs::read_to_string(&file_path) {
                    Ok(string) => {
                        println!("Read from {:?}", file_path);
                        let mnemonic = Mnemonic::parse(&string).unwrap();
                        mnemonics.push(mnemonic)
                    }
                    Err(e) => {
                        eprintln!(
                            "Faild to read file {}: {}",
                            &file_path.to_str().unwrap_or("unknown file"),
                            e
                        );
                        std::process::exit(1);
                    }
                }
            }
        }

        let mut clients: Vec<String> = Vec::new();
        for mnemonic in mnemonics.iter() {
            let xkey: ExtendedKey = mnemonic.clone().into_extended_key().unwrap();
            let xprv = xkey.into_xprv(Network::Regtest).unwrap();
            clients.push(xprv.to_string());
        }
        utils::init_client_wallet(Network::Regtest, "127.0.0.1:50001", &clients)
    }

    #[test]
    fn sign_psbt() {
        let wallets = setup_client_wallets();

        let hex_psbt = fs::read_to_string(PSBT_PATH).unwrap();
        let psbt = bdk::bitcoin::consensus::deserialize::<
            bdk::bitcoin::util::psbt::PartiallySignedTransaction,
        >(
            &<Vec<u8> as bdk::bitcoin::hashes::hex::FromHex>::from_hex(&hex_psbt).unwrap()
        )
        .unwrap();

        let psbts = list_signed_txs(psbt, &wallets);
        match merge_psbts(psbts).pop() {
            Some(psbt) => {
                println!("Finalized PSBT {:?}", &serialize_hex(&psbt));
                println!(
                    "Finalized tx extracted from PSBT {:?}",
                    &serialize_hex(&psbt.extract_tx())
                );
            }
            None => println!("Can not get first item."),
        };
    }

    #[test]
    fn dump_outpoint_and_psbt_input() {
        let wallets = setup_client_wallets();

        for (i, wallet) in wallets.iter().enumerate() {
            wallet.sync(noop_progress(), None).unwrap();
            println!(
                "wallet {:?} has {:?}",
                wallet.get_address(AddressIndex::Peek(0)).unwrap(),
                wallet.get_balance().unwrap()
            );
            // TODO: select utxo to be used as Input
            let local_utxo = &wallet.list_unspent().unwrap()[0];

            match wallet.get_psbt_input(local_utxo.clone(), None, false) {
                Ok(input) => {
                    println!("UTXO found: {:?}", &input);
                    let psbt_input = serialize_hex(&input);

                    let server_payload = CoinJoinInput {
                        outpoint: local_utxo.outpoint.to_string(),
                        psbt_input: psbt_input,
                    };
                    let payload = serde_json::to_string(&server_payload).unwrap();

                    fs::create_dir_all(SERVER_INPUT_DIR).unwrap();
                    let mut file =
                        File::create(format!("{}/{}.json", SERVER_INPUT_DIR, i)).unwrap();
                    file.write_all(payload.as_bytes()).unwrap();
                }
                Err(err) => {
                    println!("Error: {:?}", err)
                }
            }
        }
    }

    #[test]
    fn dump_output() {
        let wallets = setup_client_wallets();

        for (i, wallet) in wallets.iter().enumerate() {
            wallet.sync(noop_progress(), None).unwrap();

            let new_address = wallet.get_address(AddressIndex::Peek(0)).unwrap();
            fs::create_dir_all(OUTPUT_DIR).unwrap();
            let mut file = File::create(format!("{}/{}.txt", OUTPUT_DIR, i)).unwrap();
            file.write_all(new_address.address.to_string().as_bytes())
                .unwrap();
        }
    }
}
