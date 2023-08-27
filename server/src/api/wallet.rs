use leptos::{server, ServerFnError};

#[server(MyFun, "/api", "GetJson", "balance")]
pub async fn balance() -> Result<usize, ServerFnError>{
    Ok(0)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_balance() {
        assert_eq!(balance().await.unwrap(), 0);
    }
}
