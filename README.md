Definex is a Financial market protocol that provides both liquid money markets for cross-chain assets and peer-to-peer capital markets for longer-term cryptocurrency  loans. 

## Build

Install Rust:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Install required tools:

```
./scripts/init.sh
```

Build Wasm and native node in release mode:

```
cd cli && cargo build --release
```

## Run

### Single node development chain

```
./target/release/substrate --dev 
```

## Types

```
{
    "TxHash": "H256",
    "Deposit": {
        "account_id": "AccountId",
        "tx_hash": "Option<TxHash>",
        "amount": "Balance"
    },
    "Auth": {
        "_enum": [
            "All",
            "Deposit",
            "Withdraw",
            "Refund",
            "Mark"
        ]
    },
    "BlackOrWhite": {
        "_enum": [
            "Black",
            "White"
        ]
    },
    "ExtrinsicIndex": "u32",
    "LineNumber": "u32",
    "AuctionBalance": "Balance",
    "TotalLoanBalance": "Balance",
    "CollateralBalanceAvailable": "Balance",
    "CollateralBalanceOriginal": "Balance",
    "Price": "u128",
    "PriceReport": {
        "reporter": "AccountId",
        "price": "Price"
    },
    "LoanHealth": {
        "_enum": {
            "Well": null,
            "Warning": "LTV",
            "Liquidating": "LTV",
        }
    },
    "LoanPackageStatus": {
        "_enum": [
            "Active",
            "Inactive"
        ]
    },
    "Loan": {
        "id": "LoanId",
        "who": "AccountId",
        "collateral_balance_original": "Balance",
        "collateral_balance_available": "Balance",
        "loan_balance_total": "Balance",
        "status": "LoanHealth"
    },
    "ReleaseTrigger": {
        "_enum": {
            "PhaseChange": null,
            "BlockNumber": "BlockNumber"
        }
    },
    "LTV": "u64",
    "LoanId": "u64",
    "LoanPackageId": "u64",
    "PhaseId": "u32"
}
```

