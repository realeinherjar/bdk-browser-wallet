use anyhow::Result;
use bdk_esplora::esplora_client::{AsyncClient, Builder};

/// Creates a client from a url.
pub fn create_client(network: &str) -> Result<AsyncClient> {
    let url = match network {
        "mainnet" => "https://mempool.space/api",
        "testnet" => "https://mempool.space/testnet/api",
        _ => panic!("Invalid network"),
    };
    Ok(Builder::new(url).build_async()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;

    fn is_derivationpath<T: ?Sized + 'static>(_s: &T) -> bool {
        TypeId::of::<AsyncClient>() == TypeId::of::<T>()
    }

    #[test]
    fn test_create_client_mainnet() {
        assert!(is_derivationpath(&create_client("mainnet").unwrap()));
    }

    #[test]
    fn test_create_client_testnet() {
        assert!(is_derivationpath(&create_client("testnet").unwrap()));
    }

    #[test]
    #[should_panic]
    fn test_create_client_panic() {
        create_client("foo").unwrap();
    }
}
