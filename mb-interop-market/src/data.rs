use mb_sdk::near_sdk::{
    self,
    borsh::{
        self,
        BorshDeserialize,
        BorshSerialize,
    },
    json_types::{
        U128,
        U64,
    },
    serde::{
        Deserialize,
        Serialize,
    },
    AccountId,
    Balance,
    Gas,
    Timestamp,
};

pub const ERR_LISTING_NOT_FOUND: &str = "Listing not found";
pub const ERR_OFFER_IN_PROGRESS: &str =
    "Cannot modify listing while offer is being processed";

/// Storage deposit for 1 kB of data.
pub const TEN_MILLINEAR: Balance = 10_000_000_000_000_000_000_000;

pub const MAX_LEN_PAYOUT_NEAR: u32 = 50;
pub const MAX_LEN_PAYOUT_FT: u32 = 10;
pub const LISTING_KIND_SIMPLE: &str = "simple";
pub const FT_TRANSFER_GAS: Gas = Gas(15_000_000_000_000);
pub const NFT_TRANSFER_PAYOUT_GAS: Gas = Gas(10_000_000_000_000);
pub const NFT_RESOLVE_PAYOUT_NEAR_GAS: Gas = Gas(175_000_000_000_000);
pub const NFT_RESOLVE_PAYOUT_FT_GAS: Gas = Gas(235_000_000_000_000);
// const LISTING_KIND_AUCTION: &str = "auction";

/// A listing as it is stored on the blockchain.
///
/// Storage calculation:
///
/// | Field              | Required storage                        |
/// | ------------------ | --------------------------------------- |
/// | `nft_token_id`     | 128 bytes (limited by `nft_on_approve`) |
/// | `nft_approval_id`  | 2 bytes                                 |
/// | `nft_owner_id`     | 64 bytes                                |
/// | `nft_contract_id`  | 64 bytes                                |
/// | `price`            | 16 bytes                                |
/// | `currency`         | 65 bytes                                |
/// | `created_at`       | 8 bytes                                 |
/// | `current_offer`    | 149 bytes                               |
/// | total              | 496 bytes                               |
///
/// Additionally, storing this requires a `token_key` with a maximum of 128 +
/// 64 + 3 = 195 bytes. Each lister also has one-time storages:
///
/// - `storage_deposits_by_account`: 64 (Account ID) + 16 (u128) = 80 bytes
/// - `listings_number_by_account`: 64 (Account ID) + 8 (u64) = 72 bytes
///
/// The first listing should thus has to require a total deposit of 0.00843
/// NEAR. For simplicity and to discourage stale listings, each listing is
/// required to be backed by a storage deposit of 0.01 NEAR.
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Listing {
    /// Token ID of listed NFT
    pub nft_token_id: String,
    /// Approval ID that created the listing
    pub nft_approval_id: u64,
    /// Owner of the listed NFT (at the time of listing)
    pub nft_owner_id: AccountId,
    /// NFT contract
    pub nft_contract_id: AccountId,
    /// Price either in yoctoNEAR or the atomic unit of an FT contract
    pub price: Balance,
    /// Specifies if NEAR is requested for this listing or tokens of an FT
    /// contract, in the latter case specifying the account ID of the FT
    /// contract
    pub currency: Currency,
    /// Timestamp of the block in which this listing was created (block in which
    /// `nft_on_approve` executed successfully)
    pub created_at: Timestamp,
    /// A currently executing offer. This locks up the listing for other buyers.
    /// There are instances where other smart contracts do not attach sufficient
    /// gas to a buy call, creating a "stuck offer".
    pub current_offer: Option<Offer>,
}

/// Listing as it is serializedtowards end-users. Importantly, numbers are
/// stringified to prevent JS floating-point inaccuracies. The exception to this
/// are approval IDs, as these are not stringified in the NEP-178 standard
/// either. For field descriptions see the `Listing` struct.
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ListingJson {
    pub nft_token_id: String,
    pub nft_approval_id: u64,
    pub nft_owner_id: AccountId,
    pub nft_contract_id: AccountId,
    pub price: U128,
    pub currency: String,
    pub created_at: U64,
    pub current_offer: Option<OfferJson>,
}

impl Listing {
    pub fn new(
        nft_token_id: String,
        nft_approval_id: u64,
        nft_owner_id: AccountId,
        nft_contract_id: AccountId,
        msg: CreateListingMsg,
    ) -> Self {
        Listing {
            nft_token_id,
            nft_approval_id,
            nft_owner_id,
            nft_contract_id,
            price: msg.price.into(),
            currency: msg.ft_contract.into(),
            created_at: near_sdk::env::block_timestamp(),
            current_offer: None,
        }
    }

    pub fn token_key(&self) -> String {
        format!("{}<$>{}", self.nft_contract_id, self.nft_token_id)
    }
}

impl From<Listing> for ListingJson {
    fn from(listing: Listing) -> ListingJson {
        ListingJson {
            nft_token_id: listing.nft_token_id,
            nft_approval_id: listing.nft_approval_id,
            nft_owner_id: listing.nft_owner_id,
            nft_contract_id: listing.nft_contract_id,
            price: listing.price.into(),
            currency: listing.currency.to_string(),
            created_at: listing.created_at.into(),
            current_offer: listing.current_offer.map(|offer| offer.into()),
        }
    }
}

/// An offer as it is stored on the blockchain.
///
/// Storage calculation:
///
/// | Field              | Required storage              |
/// | ------------------ | ------------------------------|
/// | `offerer_id`       | 64 bytes (64 ASCII chars max) |
/// | `amount`           | 16 bytes                      |
/// | `referrer_id`      | 65 bytes                      |
/// | `referral_cut`     | 3 bytes                       |
/// | total              | 148 bytes                     |
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Offer {
    /// The account that created the offer.
    pub offerer_id: AccountId,
    /// The amount being offered in yoctoNEAR or atomic FT units, depending on
    /// the listings currency.
    pub amount: Balance,
    /// Affiliate through which the offer has been made.
    pub referrer_id: Option<AccountId>,
    /// Percentage that will be split between Mintbase and the affiliate on
    /// successful transaction.
    pub referral_cut: Option<u16>,
}

/// An offer as it is serialized towards the end user. Numbers are stringified
/// to prevent JS floating-point inaccuracies. Not needed on `u16` as JS can
/// represent that accurately.
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct OfferJson {
    pub offerer_id: AccountId,
    pub amount: U128,
    pub referrer_id: Option<AccountId>,
    pub referral_cut: Option<u16>,
}

impl From<Offer> for OfferJson {
    fn from(offer: Offer) -> OfferJson {
        OfferJson {
            offerer_id: offer.offerer_id,
            amount: offer.amount.into(),
            referrer_id: offer.referrer_id,
            referral_cut: offer.referral_cut,
        }
    }
}

/// Enum to hold payment methods, which can be either native NEAR, or fungible
/// tokens on NEAR protocol.
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub enum Currency {
    Near,
    FtContract(AccountId),
}

impl Currency {
    pub fn is_near(&self) -> bool {
        matches!(self, Currency::Near)
    }

    pub fn get_ft_contract_id(&self) -> Option<AccountId> {
        match self {
            Currency::FtContract(acc) => Some(acc.clone()),
            _ => None,
        }
    }
}

impl From<Option<AccountId>> for Currency {
    fn from(x: Option<AccountId>) -> Currency {
        match x {
            None => Currency::Near,
            Some(acc) => Currency::FtContract(acc),
        }
    }
}

impl ToString for Currency {
    fn to_string(&self) -> String {
        match self {
            Currency::Near => "near".to_string(),
            Currency::FtContract(ft_contract_id) => {
                format!("ft::{}", ft_contract_id)
            }
        }
    }
}

/// The message that will be passed from the NFT contract to the market to
/// specify listing parameters.
#[derive(Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct CreateListingMsg {
    /// Price in either yoctoNEAR or atomic units of the FT contract.
    pub price: U128,
    /// FT contract to use. If none, the token is listed for native NEAR.
    pub ft_contract: Option<AccountId>,
}

/// The message that will be passed form the FT contract to the market to
/// specify a listing to buy.
#[derive(Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BuyWithFtMessage {
    pub nft_contract_id: AccountId,
    pub token_id: String,
    pub affiliate_id: Option<AccountId>,
}
