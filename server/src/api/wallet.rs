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
    seed: Mnemonic,
    network: Network,
    derivation_path_external: DerivationPath,
    derivation_path_internal: DerivationPath,
) -> Result<Wallet> {
    let secp = Secp256k1::new();

    // generate external and internal descriptor from mnemonic
    let (external_descriptor, _ext_keymap) =
        match descriptor!(wpkh((seed.clone(), derivation_path_external.clone())))
            .unwrap()
            .into_wallet_descriptor(&secp, network)
        {
            Ok((extended_descriptor, keymap)) => (extended_descriptor, keymap),
            Err(e) => panic!("Invalid external derivation path: {}", e),
        };
    let (internal_descriptor, _int_keymap) =
        match descriptor!(wpkh((seed.clone(), derivation_path_internal.clone())))
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

#[server(MyFun, "/api", "GetJson", "utxo")] // GetJson is a GET and will be cached
pub async fn get_utxos(seed: String, network: String) -> Result<String, ServerFnError>{
    let seed = Mnemonic::parse_in(Language::English, seed)?;
    let network = if network == "bitcoin" { Network::Bitcoin } else { Network::Testnet };
    let base_url = if network == Network::Bitcoin { "https://mempool.space/api" } else { "https://mempool.space/testnet/api" };
    let esplora_client = Builder::new(base_url).build_async()?;
    let mut wallet = create_wallet(seed, network,
            DerivationPath::from_str("m/86'/0'/0'/0").unwrap(),
            DerivationPath::from_str("m/86'/0'/0'/1").unwrap()).unwrap();
    let _ = sync_wallet(&mut wallet, &esplora_client).await;
    let utxos = wallet.list_unspent().collect::<Vec<LocalUtxo>>();
    let json = to_string(&utxos)?;
    Ok(json)
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
            Mnemonic::parse_in(Language::English, mnemonic_12).unwrap(),
            Network::Bitcoin,
            DerivationPath::from_str("m/86'/0'/0'/0").unwrap(),
            DerivationPath::from_str("m/86'/0'/0'/1").unwrap()
        ).unwrap();
        let wallet_mainnet_24 = create_wallet(
            Mnemonic::parse_in(Language::English, mnemonic_24).unwrap(),
            Network::Bitcoin,
            DerivationPath::from_str("m/86'/0'/0'/0").unwrap(),
            DerivationPath::from_str("m/86'/0'/0'/1").unwrap()
        ).unwrap();
        let wallet_testnet_12 = create_wallet(
            Mnemonic::parse_in(Language::English, mnemonic_12).unwrap(),
            Network::Testnet,
            DerivationPath::from_str("m/86'/0'/0'/0").unwrap(),
            DerivationPath::from_str("m/86'/0'/0'/1").unwrap()
        ).unwrap();
        let wallet_testnet_24 = create_wallet(
            Mnemonic::parse_in(Language::English, mnemonic_24).unwrap(),
            Network::Testnet,
            DerivationPath::from_str("m/86'/0'/0'/0").unwrap(),
            DerivationPath::from_str("m/86'/0'/0'/1").unwrap()
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
    async fn test_balance() {
        assert_eq!(balance().await.unwrap(), 0);
    }
}
