use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Uint128;

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the current balance of the given address, 0 if unset.
    /// Return type: BalanceResponse.
    Balance { address: String },
    /// Returns metadata on the contract - name, decimals, supply, etc.
    /// Return type: TokenInfoResponse.
    TokenInfo {},
    /// Only with "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    /// Return type: AllowanceResponse.
    Allowance { owner: String, spender: String },
    /// Only with "mintable" extension.
    /// Returns who can mint and the hard cap on maximum tokens after minting.
    /// Return type: MinterResponse.
    Minter {},
    /// Only with "marketing" extension
    /// Returns more metadata on the contract to display in the client:
    /// - description, logo, project url, etc.
    /// Return type: MarketingInfoResponse.
    MarketingInfo {},
    /// Only with "marketing" extension
    /// Downloads the embedded logo data (if stored on chain). Errors if no logo data stored for
    /// this contract.
    /// Return type: DownloadLogoResponse.
    DownloadLogo {},
    /// Only with "enumerable" extension (and "allowances")
    /// Returns all allowances this owner has approved. Supports pagination.
    /// Return type: AllAllowancesResponse.
    AllAllowances {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Only with "enumerable" extension
    /// Returns all accounts that have balances. Supports pagination.
    /// Return type: AllAccountsResponse.
    AllAccounts {
        start_after: Option<String>,
        limit: Option<u32>,
    },

    // EIP4626 specific functions
    /// Returns info about the underlying asset
    Asset {},
    /// Returns the total number of underlying assets backing the token
    TotalAssets {},
    /// Converts the asset to shares of the token
    ConvertToShares { assets: Uint128 },
    /// Converts the shares of the token to assets
    ConvertToAssets { shares: Uint128 },
    /// Converts the shares of the token to assets
    MaxDeposit { receiver: String },
    /// Converts the shares of the token to assets
    PreviewDeposit { assets: Uint128 },
    /// Maximum amount that can be minted to someone in one call
    MaxMint { receiver: String },
    /// Allows to preview what happens when minting
    PreviewMint { shares: Uint128 },
    /// Maximum amount that can be withdrawn (in assets) to someone in one call
    MaxWithdraw { owner: String },
    /// Allows to preview what happens when withdrawing funds (exactly the indicated number of underlying asset)
    PreviewWithdraw { assets: Uint128 },
    /// Maximum amount that can be redeemed (in shares) to someone in one call
    MaxRedeem { owner: String },
    /// Allows to preview what happens when redeeming funds (exactly the indicated number of pool shares)
    PreviewRedeem { shares: Uint128 },
}
