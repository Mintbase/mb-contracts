import avaTest from "ava";
import { NEAR, failPromiseRejection, mintingDeposit } from "./utils/index.js";
import { MB_VERSION, setup } from "./setup.js";
import { NearAccount } from "near-workspaces";

const test = setup(avaTest);

const mint = async ({
  alice,
  store,
  metadata,
}: {
  alice: NearAccount;
  store: NearAccount;
  metadata: Record<string, any>;
}): Promise<string> => {
  if (MB_VERSION == "v1") {
    await alice.call(
      store,
      "nft_batch_mint",
      {
        owner_id: alice.accountId,
        metadata,
        num_to_mint: 2,
      },
      { attachedDeposit: mintingDeposit({ n_tokens: 1, n_splits: 20 }) }
    );
    return "0";
  }

  await alice.call(
    store,
    "create_metadata",
    { metadata, price: NEAR(0.01) },
    { attachedDeposit: NEAR(0.1) }
  );

  await alice.call(
    store,
    "mint_on_metadata",
    {
      metadata_id: "0",
      num_to_mint: 2,
      owner_id: alice.accountId,
    },
    { attachedDeposit: NEAR(0.05) }
  );

  return "0:0";
};

test("metadata", async (test) => {
  const { alice, store } = test.context.accounts;
  test.deepEqual(await store.view("nft_metadata"), {
    base_uri: null,
    icon: null,
    name: "alice",
    reference: null,
    reference_hash: null,
    spec: "nft-1.0.0",
    symbol: "ALICE",
  });

  const tokenId = await mint({
    alice,
    store,
    metadata: {
      title: "Yadda",
      description: "Yadda, yadda!",
      reference: "reference",
      reference_hash: "cmVmZXJlbmNl",
      media: "media",
      media_hash: "bWVkaWE=",
      starts_at: "1672531200000000000",
      expires_at: "1672531200000000000",
      extra: "No more extras for you!",
    },
  }).catch(failPromiseRejection(test, "minting"));

  test.deepEqual(
    await store.view("nft_token_metadata", { token_id: tokenId }),
    {
      copies: 2, // this is automagically inserted because we minted 2 :)
      title: "Yadda",
      description: "Yadda, yadda!",
      reference: "reference",
      reference_hash: "cmVmZXJlbmNl",
      media: "media",
      media_hash: "bWVkaWE=",
      starts_at: "1672531200000000000",
      expires_at: "1672531200000000000",
      extra: "No more extras for you!",
    }
  );

  // TODO::testing::low: deploying with icon/base URI
  // TODO::testing::low: changing icon/base URI
});
