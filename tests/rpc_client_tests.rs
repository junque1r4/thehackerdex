use hackerdex::rpc::RateLimitedClient;
use std::env;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_client_initialization() {
    let client = RateLimitedClient::new(None);
    assert!(
        !client.endpoint().is_empty(),
        "RPC endpoint should not be empty"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_client_custom_endpoint() {
    let custom_url = "https://solana-rpc.publicnode.com".to_string();
    let client = RateLimitedClient::new(Some(custom_url.clone()));
    assert_eq!(
        client.endpoint(),
        custom_url,
        "RPC endpoint should match the provided URL"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_version() {
    // Skip this test if CI environment variable is set to avoid hitting real RPC servers in CI
    if env::var("CI").is_ok() {
        return;
    }

    let client = RateLimitedClient::new(None);
    let result = client.get_version().await;
    assert!(result.is_ok(), "Should successfully get version info");

    if let Ok(version) = result {
        assert!(
            !version.solana_core.is_empty(),
            "Solana core version should not be empty"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_account_info_valid() {
    // Skip this test if CI environment variable is set
    if env::var("CI").is_ok() {
        return;
    }

    // Wormhole core contract - should exist
    let address = "worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth";

    let client = RateLimitedClient::new(None);
    let result = client.get_account_info(address).await;

    assert!(result.is_ok(), "Should return Ok result for valid address");

    if let Ok(account_opt) = result {
        assert!(
            account_opt.is_some(),
            "Account should exist for Wormhole address"
        );

        if let Some(account) = account_opt {
            assert!(
                account.lamports > 0,
                "Account should have non-zero lamports"
            );
            assert!(
                !account.data.is_empty(),
                "Account should have non-empty data"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_account_info_invalid() {
    // Invalid address format
    let address = "not_a_valid_solana_address";

    let client = RateLimitedClient::new(None);
    let result = client.get_account_info(address).await;

    assert!(
        result.is_err(),
        "Should return error for invalid address format"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_signatures() {
    // Skip this test if CI environment variable is set
    if env::var("CI").is_ok() {
        return;
    }

    // Jupiter aggregator - should have transactions
    let address = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

    let client = RateLimitedClient::new(None);
    let result = client
        .get_signatures_for_address(address, None, None, Some(5))
        .await;

    assert!(result.is_ok(), "Should successfully get signatures");

    if let Ok(signatures) = result {
        assert!(
            !signatures.is_empty(),
            "Should return signatures for Jupiter address"
        );
        assert!(signatures.len() <= 5, "Should respect the limit parameter");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_transaction() {
    if env::var("CI").is_ok() {
        return;
    }

    // wallet address
    let address = "EATYKPBH2wUsSx3HbqVgiJWVVCkr6ZNJ5g6AVZWY7JHS";

    let client = RateLimitedClient::new(None);
    let signatures_result = client
        .get_signatures_for_address(address, None, None, Some(1))
        .await;

    assert!(
        signatures_result.is_ok(),
        "Should get signatures successfully"
    );

    if let Ok(signatures) = signatures_result {
        if !signatures.is_empty() {
            let signature = &signatures[0].signature;
            let tx_result = client.get_transaction(signature).await;

            assert!(tx_result.is_ok(), "Should get transaction successfully");

            if let Ok(tx_opt) = tx_result {
                assert!(tx_opt.is_some(), "Transaction should exist");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_transaction_invalid() {
    // Invalid transaction signature format
    let signature = "not_a_valid_signature";

    let client = RateLimitedClient::new(None);
    let result = client.get_transaction(signature).await;

    assert!(
        result.is_err(),
        "Should return error for invalid signature format"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_get_balance() {
    // Skip this test if CI environment variable is set
    if env::var("CI").is_ok() {
        return;
    }

    // Wormhole core contract - should have balance
    let address = "worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth";

    let client = RateLimitedClient::new(None);
    let result = client.get_balance(address).await;
    if let Ok(balance) = result {
        println!("Balance: {}", balance);
    }
    assert!(result.is_ok(), "Should successfully get balance");

    if let Ok(balance) = result {
        assert!(balance > 0, "Wormhole address should have non-zero balance");
    }
}
