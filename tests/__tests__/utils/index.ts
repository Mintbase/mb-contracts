import { NearAccount } from "near-workspaces";
import { ExecutionContext } from "ava";
import { NEAR, mintingDeposit } from "./balances.js";
import { CHANGE_SETTING_VERSION, MB_VERSION } from "../setup.js";

// TODO::testing::low: commenting all my test utils

export * from "./balances.js";
export * from "./panics.js";
export * from "./token.js";
export * from "./approvals.js";
export * from "./events.js";
export * from "./payouts.js";
export * from "./download-contracts.js";

// ---------------------------------- misc ---------------------------------- //
function parseEvent(log: string) {
  if (log.slice(0, 11) !== "EVENT_JSON:")
    throw new Error(`${log}: Not an event log`);
  return JSON.parse(log.slice(11).trimStart());
}

export async function batchMint({
  owner,
  store,
  owner_id,
  num_to_mint,
}: {
  owner: NearAccount;
  store: NearAccount;
  num_to_mint: number;
  owner_id?: string;
}): Promise<string[]> {
  if (!owner_id) owner_id = owner.accountId;

  if (MB_VERSION == "v1") {
    const mintCall = await owner.callRaw(
      store,
      "nft_batch_mint",
      {
        owner_id,
        num_to_mint,
        metadata: {},
      },
      {
        attachedDeposit: mintingDeposit({ n_tokens: num_to_mint }),
      }
    );

    return parseEvent(mintCall.logs[0]).data[0].token_ids;
  }

  await owner.call(
    store,
    "create_metadata",
    { metadata: {}, price: NEAR(0.01) },
    { attachedDeposit: NEAR(0.1) }
  );

  const mintCall = await owner.callRaw(
    store,
    "mint_on_metadata",
    {
      metadata_id: "0",
      num_to_mint: 2,
      owner_id,
    },
    { attachedDeposit: NEAR(0.05) }
  );

  return parseEvent(mintCall.logs[0]).data[0].token_ids;
}

export async function prepareTokenListing(
  test: ExecutionContext,
  accounts: Record<string, NearAccount>
) {
  const { alice, store, market, factory } = accounts;
  await batchMint({ owner: alice, store, num_to_mint: 2 }).catch(
    failPromiseRejection(test, "minting")
  );

  await market
    .call(
      market,
      "update_allowlist",
      { account_id: factory.accountId, state: true },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection(test, "allowing store on market"));
}

// TODO::testing::low: use this function consistently
export function failPromiseRejection(
  test: ExecutionContext,
  msg: string
): (e: any) => void {
  return (e: any) => {
    test.log(`Promise rejected while ${msg}:`);
    test.log(e);
    test.fail();
  };
}

export function hours(x: number): number {
  return Math.round(x * 3600 * 1e9);
}

export function changeSettingsData(subset: Record<string, string>) {
  const data: Record<string, string | null> = {
    granted_minter: null,
    revoked_minter: null,
    new_icon_base64: null,
    new_owner: null,
    new_base_uri: null,
  };

  if (CHANGE_SETTING_VERSION === "0.2.0") {
    data.allow_open_minting = null;
    data.set_minting_cap = null;
  }

  Object.keys(subset).forEach((k) => {
    data[k] = subset[k];
  });

  return data;
}
