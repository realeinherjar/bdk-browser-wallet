use anyhow::Result;
use bdk::{
    Wallet,
    bitcoin::{Network, util::bip32::DerivationPath, secp256k1::Secp256k1, psbt::PartiallySignedTransaction, Transaction, Address},
    keys::bip39::{Mnemonic, Language},
    descriptor,
    descriptor::IntoWalletDescriptor, LocalUtxo, wallet::{AddressIndex, AddressInfo}, FeeRate, SignOptions,
};
use bdk_esplora::{esplora_client::{AsyncClient, Builder}, EsploraAsyncExt};
use leptos::{server, ServerFnError};
use std::{str::FromStr, u32, collections::HashMap};
use serde::{Serialize, Deserialize};
use serde_json::to_string;

// NOTE: hardcoded to BIP86
const DEFAULT_DERIVATION_PATH_EXTERNAL: &str = "m/86'/0'/0'/0";
const DEFAULT_DERIVATION_PATH_INTERNAL: &str = "m/86'/0'/0'/1";

// NOTE: hardcoded to mempool.space
const DEFAULT_ESPLORA_BASE_URL_MAINNET: &str = "https://mempool.space/api";
const DEFAULT_ESPLORA_BASE_URL_TESTNET: &str = "https://mempool.space/testnet/api";

#[derive(Debug)]
enum AddressType {
    Receive,
    Change
}

/// Hack to get around the fact that BDK's AddressInfo doesn't implement Serialize.
#[derive(Debug, Serialize, Deserialize)]
struct AddressInfoDef {
    index: usize,
    address: String,
    keychain: String,
}
impl AddressInfoDef {
    fn from(address_info: AddressInfo) -> Self {
        Self {
            index: address_info.index as usize,
            address: address_info.address.to_string(),
            keychain: format!("{:?}", address_info.keychain),
        }
    }
}

/// Creates a wallet from a mnemonic, a network type, and an internal and external derivation paths.
pub fn create_wallet(
    mnemonic: &str,
    network: &str,
    derivation_path_external: &str,
    derivation_path_internal: &str,
) -> Result<Wallet> {
    let secp = Secp256k1::new();

    let mnemonic = Mnemonic::parse_in(Language::English, mnemonic)?;
    let network = match network {
        "mainnet" => Network::Bitcoin,
        "testnet" => Network::Testnet,
        "signet" => Network::Signet,
        "regtest" => Network::Regtest,
        &_ => Network::Testnet, // NOTE: a good default
    };

    // generate derivation paths
    let external_path = DerivationPath::from_str(derivation_path_external).unwrap();
    let internal_path = DerivationPath::from_str(derivation_path_internal).unwrap();

    // generate external and internal descriptor from mnemonic
    let (external_descriptor, _ext_keymap) =
        match descriptor!(tr((mnemonic.clone(), external_path))) // NOTE: taproot is hardcoded tr
            .unwrap()
            .into_wallet_descriptor(&secp, network)
        {
            Ok((extended_descriptor, keymap)) => (extended_descriptor, keymap),
            Err(e) => panic!("Invalid external derivation path: {}", e),
        };
    let (internal_descriptor, _int_keymap) =
        match descriptor!(tr((mnemonic.clone(), internal_path))) // NOTE: taproot is hardcoded tr
            .unwrap()
            .into_wallet_descriptor(&secp, network)
        {
            Ok((extended_descriptor, keymap)) => (extended_descriptor, keymap),
            Err(e) => panic!("Invalid internal derivation path: {}", e),
        };

    Ok(Wallet::new_no_persist(external_descriptor, Some(internal_descriptor), network)?)
}

/// Sync a wallet with the Esplora client.
pub async fn sync_wallet(wallet: &mut Wallet, client: &AsyncClient) -> Result<bool> {
    let local_chain = wallet.checkpoints();

    let keychain_spks = wallet.spks_of_all_keychains().into_iter().collect();
    let update = client
        .scan(
            local_chain,
            keychain_spks,
            [],
            [],
            5, // stop gap
            5, // parallel requests
        )
        .await?;
    wallet.apply_update(update)?;
    Ok(wallet.commit()?)
}

/// Get the fee estimates from the Esplora server.
/// The default block is 1, which is the next block.
pub async fn get_fee_estimates(client: &AsyncClient, block: Option<usize>) -> Result<f32> {
    let fee_estimates: HashMap<String, f64> = client.get_fee_estimates().await?;

    // NOTE: if block is not specified, use the next block
    let fee_estimate = match block {
        Some(block) => fee_estimates.get(&block.to_string()).unwrap(),
        None => fee_estimates.get("1").unwrap(),
    };
    Ok(*fee_estimate as f32)
}

/// Create a Signed Transaction from a wallet using all available coins to send to a given address.
/// Estimate the fee using the Esplora client.
/// Tries to use fee rate such that it will be included in the next block.
/// By default, the transaction is marked as RBF.
pub async fn create_signed_transaction(
    wallet: &mut Wallet,
    address: &str,
    client: &AsyncClient,
) -> Result<PartiallySignedTransaction> {
    let fee_rate = get_fee_estimates(client, None).await.unwrap();
    let address = Address::from_str(address)?;

    // create a drain transaction
    let mut tx_builder = wallet.build_tx();
    tx_builder
        // Spend all outputs in this wallet.
        .drain_wallet()
        // Send the excess (which is all the coins minus the fee) to this address.
        .drain_to(address.script_pubkey())
        .fee_rate(FeeRate::from_sat_per_vb(fee_rate))
        .enable_rbf();

    let (mut psbt, _) = match tx_builder.finish() {
        Ok(psbt) => psbt,
        Err(e) => panic!("Error creating transaction: {}", e),
    };
    match wallet.sign(&mut psbt, SignOptions::default()) {
        Ok(finalized) => finalized,
        Err(e) => panic!("Error signing transaction: {}", e),
    };
    Ok(psbt)
}

/// Broadcast a signed transaction to the network using the given Esplora client.
pub async fn broadcast_signed_transaction(psbt: PartiallySignedTransaction, client: &AsyncClient) -> Result<Transaction> {
    let tx = psbt.extract_tx();
    let _ = client.broadcast(&tx).await;
    Ok(tx)
}

/// Returns a JSON string of the wallet's utxos.
#[server(GetUtxo, "/api", "GetJson", "utxo")] // GetJson is a GET and will be cached
pub async fn get_utxo(mnemonic: String, network: String) -> Result<String, ServerFnError>{
    // Create the Esplora async client
    let base_url = if network == "bitcoin" { DEFAULT_ESPLORA_BASE_URL_MAINNET } else { DEFAULT_ESPLORA_BASE_URL_TESTNET };
    let esplora_client = Builder::new(base_url).build_async()?;

    // Create the wallet
    let mut wallet = create_wallet(mnemonic.as_str(), network.as_str(),
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
            ).unwrap();

    // Sync Wallet
    let _ = sync_wallet(&mut wallet, &esplora_client).await;

    // Get UTXOs
    let utxo = wallet.list_unspent().collect::<Vec<LocalUtxo>>();

    // Serialize to JSON
    let json = to_string(&utxo)?;
    Ok(json)
}

/// Returns a JSON string of the wallet's balance.
#[server(GetBalance, "/api", "GetJson", "balance")] // GetJson is a GET and will be cached
pub async fn get_balance(mnemonic: String, network: String) -> Result<String, ServerFnError> {
    // Create the Esplora async client
    let base_url = if network == "bitcoin" { DEFAULT_ESPLORA_BASE_URL_MAINNET } else { DEFAULT_ESPLORA_BASE_URL_TESTNET };
    let esplora_client = Builder::new(base_url).build_async()?;

    // Create the wallet
    let mut wallet = create_wallet(mnemonic.as_str(), network.as_str(),
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
            ).unwrap();

    // Sync Wallet
    let _ = sync_wallet(&mut wallet, &esplora_client).await;

    // Get Balance
    let balance = wallet.get_balance();

    // Serialize to JSON
    let json = to_string(&balance)?;
    Ok(json)
}

/// Returns a JSON string of the wallet's address for a given address type and index.
/// Address type can be "receive" or "change".
#[server(GetAddress, "/api", "GetJson", "address")] // GetJson is a GET and will be cached
pub async fn get_address(mnemonic: String, network: String, address_type: String, index: usize) -> Result<String, ServerFnError> {
    // Address wrangling
    let address_type = address_type.as_str();
    let address_type: AddressType = match address_type {
        "receive" => AddressType::Receive,
        "change" => AddressType::Change,
        &_ => AddressType::Receive, // NOTE: a good default
    };
    let address_index: AddressIndex = AddressIndex::Peek(index as u32);

    // Create the wallet
    let mut wallet = create_wallet(mnemonic.as_str(), network.as_str(),
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
            ).unwrap();

    // Get the address
    let address = match address_type {
        AddressType::Receive => wallet.get_address(address_index),
        AddressType::Change => wallet.get_internal_address(address_index),
    };
    let address = AddressInfoDef::from(address);

    // Serialize to JSON
    let json = to_string(&address)?;
    Ok(json)
}


/// Returns a JSON string of the wallet's balance.
#[server(PostSendTransaction, "/api", "Url", "send")]
pub async fn post_send_transaction(mnemonic: String, network: String, address: String) -> Result<String, ServerFnError> {
// Create the Esplora async client
    let base_url = if network == "bitcoin" { DEFAULT_ESPLORA_BASE_URL_MAINNET } else { DEFAULT_ESPLORA_BASE_URL_TESTNET };
    let esplora_client = Builder::new(base_url).build_async()?;

    // Create the wallet
    let mut wallet = create_wallet(mnemonic.as_str(), network.as_str(),
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
            ).unwrap();

    // Sync Wallet
    let _ = sync_wallet(&mut wallet, &esplora_client).await;

    // Create a Signed Transaction
    // that drains all available coins to send to the given address
    let psbt = create_signed_transaction(&mut wallet, address.as_str(), &esplora_client).await.unwrap();

    // Broadcast the Signed Transaction
    let tx = broadcast_signed_transaction(psbt, &esplora_client).await.unwrap();

    // Serialize to JSON
    let json = to_string(&tx)?;
    Ok(json)
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::any::TypeId;

    use bdk::wallet::{AddressIndex, Wallet};
    use bdk::bitcoin::{
        Txid, Transaction, PackedLockTime, BlockHash, TxOut,
        hashes::Hash,
    };
    use bdk_esplora::{esplora_client::{AsyncClient, Builder}, EsploraAsyncExt};
    use bdk_chain::{BlockId, ConfirmationTime};

    fn is_wallet<T: ?Sized + 'static>(_s: &T) -> bool {
       TypeId::of::<Wallet>() == TypeId::of::<T>()
    }

    fn is_psbt<T: ?Sized + 'static>(_s: &T) -> bool {
        TypeId::of::<PartiallySignedTransaction>() == TypeId::of::<T>()
    }

    /// Return a fake wallet that appears to be funded for testing.
    pub fn get_funded_wallet_with_change(
        mnemonic: &str,
        derivation_path_external: &str,
        derivation_path_internal: &str,
    ) -> (Wallet, Txid) {
        let mut wallet = create_wallet(
            mnemonic,
            "regtest",
            derivation_path_external,
            derivation_path_internal,
        ).unwrap();

        let address = wallet.get_address(AddressIndex::New).address;

        let tx = Transaction {
            version: 1,
            lock_time: PackedLockTime(0),
            input: vec![],
            output: vec![TxOut {
                value: 50_000,
                script_pubkey: address.script_pubkey(),
            }],
        };

        wallet
            .insert_checkpoint(BlockId {
                height: 1_000,
                hash: BlockHash::all_zeros(),
            })
            .unwrap();
        wallet
            .insert_tx(
                tx.clone(),
                ConfirmationTime::Confirmed {
                    height: 1_000,
                    time: 100,
                },
            )
            .unwrap();

        (wallet, tx.txid())
    }

    #[test]
    fn test_create_wallet(){
        let mnemonic_12 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon cactus";
        let mnemonic_24 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let wallet_mainnet_12 = create_wallet(
            mnemonic_12,
            "mainnet",
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
        ).unwrap();
        let wallet_mainnet_24 = create_wallet(
            mnemonic_24,
            "mainnet",
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
        ).unwrap();
        let wallet_testnet_12 = create_wallet(
            mnemonic_12,
            "testnet",
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
        ).unwrap();
        let wallet_testnet_24 = create_wallet(
            mnemonic_24,
            "testnet",
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
        ).unwrap();

        assert!(is_wallet(&wallet_mainnet_12));
        assert!(is_wallet(&wallet_mainnet_24));
        assert!(is_wallet(&wallet_testnet_12));
        assert!(is_wallet(&wallet_testnet_24));
        assert_eq!(wallet_mainnet_12.network(), Network::Bitcoin);
        assert_eq!(wallet_mainnet_24.network(), Network::Bitcoin);
        assert_eq!(wallet_testnet_12.network(), Network::Testnet);
        assert_eq!(wallet_testnet_24.network(), Network::Testnet);
    }
    #[tokio::test]
    async fn test_create_signed_transaction() {
        let mnemonic_24 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

        let (mut wallet, _txid) = get_funded_wallet_with_change(
            mnemonic_24,
            DEFAULT_DERIVATION_PATH_EXTERNAL,
            DEFAULT_DERIVATION_PATH_INTERNAL,
        );
 
        let address_mainnet = "tb1prgvu88s0074nqgq8z95uq250lx4pken99yxerwz5mrcjhrzq642s6l247d";
        let address_testnet = "tb1pce9rpv8x32r4y6xe0063kav2rpp8x9yquhvyjnfmzlk3zqn2rvuq5x7c7c";
 
        let esplora_mainnet = Builder::new(DEFAULT_ESPLORA_BASE_URL_MAINNET).build_async().unwrap();
        let esplora_testnet =Builder::new(DEFAULT_ESPLORA_BASE_URL_TESTNET).build_async().unwrap();
 
        let psbt_mainnet =
            create_signed_transaction(&mut wallet, address_mainnet, &esplora_mainnet).await.unwrap();
        let psbt_testnet =
            create_signed_transaction(&mut wallet, address_testnet, &esplora_testnet).await.unwrap();
 
        assert!(is_psbt(&psbt_mainnet));
        assert!(is_psbt(&psbt_testnet));
    }
}
