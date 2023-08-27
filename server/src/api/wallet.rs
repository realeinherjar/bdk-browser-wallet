use anyhow::Result;
use bdk::{
    Wallet,
    bitcoin::{Network,util::bip32::DerivationPath, secp256k1::Secp256k1},
    keys::bip39::Mnemonic,
    descriptor,
    descriptor::IntoWalletDescriptor,
};
use leptos::{server, ServerFnError};

#[server(MyFun, "/api", "GetJson", "balance")]
pub async fn balance() -> Result<usize, ServerFnError>{
    Ok(0)
}


#[cfg(test)]
mod tests {
    use super::*;


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
