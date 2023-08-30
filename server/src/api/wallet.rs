use anyhow::Result;
use bdk::{
    Wallet,
    bitcoin::{Network,util::bip32::DerivationPath, secp256k1::Secp256k1},
    keys::bip39::{Mnemonic, Language},
    descriptor,
    descriptor::IntoWalletDescriptor, LocalUtxo,
};
use bdk_esplora::{esplora_client::{AsyncClient, Builder}, EsploraAsyncExt};
use leptos::{server, ServerFnError};
use std::str::FromStr;
use serde_json::to_string;

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

/// Returns a JSON string of the wallet's utxos.
#[server(GetUtxo, "/api", "GetJson", "utxo")] // GetJson is a GET and will be cached
pub async fn get_utxo(mnemonic: String, network: String) -> Result<String, ServerFnError>{
    // Create the Esplora async client
    let base_url = if network == "bitcoin" { "https://mempool.space/api" } else { "https://mempool.space/testnet/api" };
    let esplora_client = Builder::new(base_url).build_async()?;

    // Create the wallet
    // NOTE: hardcoded to BIP86
    let mut wallet = create_wallet(mnemonic.as_str(), network.as_str(),
            "m/86'/0'/0'/0",
            "m/86'/0'/0'/1").unwrap();

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
    let base_url = if network == "bitcoin" { "https://mempool.space/api" } else { "https://mempool.space/testnet/api" };
    let esplora_client = Builder::new(base_url).build_async()?;

    // Create the wallet
    // NOTE: hardcoded to BIP86
    let mut wallet = create_wallet(mnemonic.as_str(), network.as_str(),
            "m/86'/1'/0'/0",
            "m/86'/1'/0'/1").unwrap();

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
#[server(GetAdress, "/api", "GetJson", "address")] // GetJson is a GET and will be cached
pub async fn get_address(mnemonic: String, network: String, address_type: &str, index: usize) -> Result<String, ServerFnError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::any::TypeId;

    #[test]
    fn test_create_wallet(){
        let mnemonic_12 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon cactus";
        let mnemonic_24 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let wallet_mainnet_12 = create_wallet(
            mnemonic_12,
            "mainnet",
            "m/86'/0'/0'/0",
            "m/86'/0'/0'/1",
        ).unwrap();
        let wallet_mainnet_24 = create_wallet(
            mnemonic_24,
            "mainnet",
            "m/86'/0'/0'/0",
            "m/86'/0'/0'/1",
        ).unwrap();
        let wallet_testnet_12 = create_wallet(
            mnemonic_12,
            "testnet",
            "m/86'/0'/0'/0",
            "m/86'/0'/0'/1",
        ).unwrap();
        let wallet_testnet_24 = create_wallet(
            mnemonic_24,
            "testnet",
            "m/86'/0'/0'/0",
            "m/86'/0'/0'/1",
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
}
