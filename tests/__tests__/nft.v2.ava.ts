import avaTest from "ava";
import { BN, NearAccount, TransactionResult } from "near-workspaces";
import {
  assertEventLogs,
  failPromiseRejection,
  mintingDeposit,
  changeSettingsData,
  assertContractPanic,
  NEAR,
  Tgas,
} from "./utils/index.js";
import {
  setup,
  MB_VERSION,
  CHANGE_SETTING_VERSION,
  createAndDeploy,
} from "./setup.js";

const test = setup(avaTest);

const createMetadata = async ({
  alice,
  store,
  args,
}: {
  alice: NearAccount;
  store: NearAccount;
  args: Record<string, any>;
}) => {
  const call = await alice.callRaw(store, "create_metadata", args, {
    attachedDeposit: NEAR(0.1),
  });
  if (call.failed) throw new Error(JSON.stringify(call));
  return call;
};
const mintOnMetadata = async ({
  bob,
  store,
  args,
  deposit,
}: {
  bob: NearAccount;
  store: NearAccount;
  args: Record<string, any>;
  deposit: number;
}) => {
  const depositCall = await bob.callRaw(
    store,
    "deposit_storage",
    { args },
    { attachedDeposit: NEAR(0.05) }
  );
  if (depositCall.failed) throw new Error(JSON.stringify(depositCall));

  const mintCall = await bob.callRaw(store, "mint_on_metadata", args, {
    attachedDeposit: NEAR(deposit),
  });
  if (mintCall.failed) throw new Error(JSON.stringify(mintCall));
  return mintCall;
};

const mint = async ({ store, alice, bob }: Record<string, NearAccount>) => {
  await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      metadata_id: "1",
      price: NEAR(0.01),
    },
  });

  const split_owners = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 6000;
    o["b.near"] = 4000;
    return o;
  })();

  await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "1",
      owner_id: bob.accountId,
      token_ids: ["1"],
      split_owners,
    },
    deposit: 0.05,
  });
};

test("v2::reset_splits", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;

  await mint({ store, alice, bob }).catch(
    failPromiseRejection(test, "minting")
  );

  const payout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "6000000000000000";
    p["b.near"] = "4000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: "1:1",
      balance: "10000000000000000",
    }),
    { payout }
  );

  const newSplitOwners = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 4000;
    o["b.near"] = 6000;
    return o;
  })();

  const setSplitsCall = await bob.callRaw(
    store,
    "set_split_owners",
    {
      token_ids: ["1:1"],
      split_between: newSplitOwners,
    },
    { attachedDeposit: mintingDeposit({ n_tokens: 1, n_splits: 2 }) }
  );

  assertEventLogs(
    test,
    (setSplitsCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "0.1.0",
        event: "nft_set_split_owners",
        data: {
          split_owners: {
            "a.near": 4000,
            "b.near": 6000,
          },
          token_ids: ["1:1"],
        },
      },
    ],
    "resetting splits"
  );

  const newPayout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "4000000000000000";
    p["b.near"] = "6000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: "1:1",
      balance: "10000000000000000",
    }),
    { payout: newPayout }
  );
});

test("v2::minting_cap", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, store } = test.context.accounts;
  await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      metadata_id: "1",
      price: NEAR(0.01),
    },
  });

  // No minting cap exists initially
  test.is(await store.view("get_minting_cap"), null);

  // Setting minting cap works
  const setMintingCapCall = await alice.callRaw(
    store,
    "set_minting_cap",
    { minting_cap: 2 },
    { attachedDeposit: "1" }
  );

  assertEventLogs(
    test,
    (setMintingCapCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({
          set_minting_cap: "2",
        }),
      },
    ],
    "setting minting cap"
  );

  // New minting cap is successfuly returned
  test.is(await store.view("get_minting_cap"), 2);

  // cannot set minting cap again
  await assertContractPanic(
    test,
    async () => {
      await alice.call(
        store,
        "set_minting_cap",
        { minting_cap: 20 },
        { attachedDeposit: "1" }
      );
    },
    "Minting cap has already been set",
    "Minting cap reset"
  );

  // try to mint beyond cap
  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob: alice,
        store,
        args: {
          metadata_id: "1",
          owner_id: alice.accountId,
          num_to_mint: 3,
        },
        deposit: 0.05,
      });
    },
    "This mint would exceed the smart contracts minting cap",
    "Minting beyond cap"
  );
  // TODO: (low priority) cannot set cap beyond already minted tokens
  // TODO: (low priority) requires yoctoNEAR deposit
});

test("v2::create_metadata", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;

  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at: null,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata"
  );

  // create metadata with explicit metadata ID
  const createMetadataCall1 = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      metadata_id: "12",
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall1 as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "12",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at: null,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata with explicit metadata ID"
  );
});

test("v2::mint_on_metadata", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;
  await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      price: NEAR(0.01),
    },
  });

  const mintOnMetadataCall = await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 3,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });

  assertEventLogs(
    test,
    (mintOnMetadataCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: bob.accountId,
            token_ids: ["0:0", "0:1", "0:2"],
            // TODO: should the minter here be alice?
            memo: '{"royalty":null,"split_owners":null,"meta_id":null,"meta_extra":null,"minter":"bob.test.near"}',
          },
        ],
      },
    ],
    "minting on metadata metadata"
  );

  const mintOnMetadataCall1 = await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      token_ids: ["12"],
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });
  assertEventLogs(
    test,
    (mintOnMetadataCall1 as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: bob.accountId,
            token_ids: ["0:12"],
            memo: '{"royalty":null,"split_owners":null,"meta_id":null,"meta_extra":null,"minter":"bob.test.near"}',
          },
        ],
      },
    ],
    "minting on metadata metadata"
  );

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          token_ids: ["12"],
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    "Token with ID 0:12 already exist",
    "Minting token ID twice"
  );

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.005,
      });
    },
    "Attached deposit does not cover the total price of 10000000000000000000000 yoctoNEAR",
    "Minting with insufficient deposit"
  );
});

test("v2::minters_allowlist", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;

  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      minters_allowlist: [bob.accountId],
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: [bob.accountId],
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at: null,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata"
  );

  // bob can mint
  await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 1,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob: alice,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    `${alice.accountId} is not allowed to mint or has already minted this metadata`,
    "Non-allowlisted account could mint"
  );
});

test("v2::minters_allowlist_unique_minters", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;

  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      minters_allowlist: [bob.accountId],
      unique_minters: true,
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: [bob.accountId],
          unique_minters: true,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at: null,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata with unique minters"
  );

  // bob can mint
  await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 1,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    `${bob.accountId} is not allowed to mint or has already minted this metadata`,
    "Minting twice with same account"
  );
});

test("v2::royalties", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;
  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      royalty_args: {
        split_between: { "a.near": 6000, "b.near": 4000 },
        percentage: 2000,
      },
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: {
            percentage: { numerator: 2000 },
            split_between: {
              "a.near": { numerator: 6000 },
              "b.near": { numerator: 4000 },
            },
          },
          max_supply: null,
          starts_at: null,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata with royalties"
  );

  const mintOnMetadataCall = await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 3,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });

  assertEventLogs(
    test,
    (mintOnMetadataCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: bob.accountId,
            token_ids: ["0:0", "0:1", "0:2"],
            memo: '{"royalty":{"split_between":{"a.near":{"numerator":6000},"b.near":{"numerator":4000}},"percentage":{"numerator":2000}},"split_owners":null,"meta_id":null,"meta_extra":null,"minter":"bob.test.near"}',
          },
        ],
      },
    ],
    "minting on metadata metadata"
  );

  test.deepEqual(
    await store.view("nft_payout", {
      token_id: "0:0",
      balance: "10000000000000000",
    }),
    {
      payout: {
        "a.near": "1200000000000000",
        "b.near": "800000000000000",
        "bob.test.near": "8000000000000000",
      },
    }
  );
});

test("v2::per_metadata_max_supply", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;
  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      max_supply: 1,
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: 1,
          starts_at: null,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata with royalties"
  );

  await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 1,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  }).catch(failPromiseRejection(test, "minting within max supply"));
  // should be successful

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    "This mint would exceed the metadatas minting cap",
    "Minting beyong max_supply"
  );
});

test("v2::metadata_expiry", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const expires_at = (Date.now() - 1000).toString();
  const { alice, bob, store } = test.context.accounts;
  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      expires_at,
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at: null,
          expires_at,
          is_locked: true,
        },
      },
    ],
    "creating metadata with expiry"
  );

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    "This metadata has expired and can no longer be minted on",
    "Minting after metadata expiry"
  );
});

test("v2::metadata_start", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const starts_at = ((Date.now() + 100e3) * 1e6).toString();
  const { alice, bob, store } = test.context.accounts;
  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      starts_at,
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at,
          expires_at: null,
          is_locked: true,
        },
      },
    ],
    "creating metadata with start time"
  );

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    "This metadata has not yet started and cannot be minted on",
    "Minting before metadata start"
  );
});

test("v2::dynamic_nfts", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;
  const getMedia = async (token_id: string): Promise<string> => {
    const token: { metadata: { media: string } } = await store.view(
      "nft_token",
      { token_id }
    );
    return token.metadata.media;
  };

  const createMetadataCall = await createMetadata({
    alice,
    store,
    args: {
      metadata: { media: "foo" },
      is_dynamic: true,
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: "0",
          minters_allowlist: null,
          unique_minters: false,
          price: NEAR(0.01).toString(),
          ft_contract_id: null,
          royalty: null,
          max_supply: null,
          starts_at: null,
          expires_at: null,
          is_locked: false,
        },
      },
    ],
    "creating dynamic metadata"
  );

  // mint some normally
  await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 1,
      owner_id: bob.accountId,
    },
    deposit: 0.02,
  });
  // mint some with specified token ID
  await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      token_ids: ["12"],
      owner_id: bob.accountId,
    },
    deposit: 0.02,
  });

  // assert that media is foo
  const oldMedia = await getMedia("0:12");
  test.is(oldMedia, "foo");

  // update metadata
  const updateMetadataCall = await alice.callRaw(
    store,
    "update_metadata",
    { metadata_id: "0", metadata: { media: "bar" } },
    { attachedDeposit: "1" }
  );
  assertEventLogs(
    test,
    (updateMetadataCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.2.0",
        event: "nft_metadata_update",
        data: [{ token_ids: ["0:0", "0:12"] }],
      },
    ],
    "updating metadata"
  );
  // check that metadata has changed on the smart contract
  const newMedia = await getMedia("0:12");
  test.is(newMedia, "bar");

  // alice cannot update without yocto deposit
  await assertContractPanic(
    test,
    async () => {
      await alice.call(store, "update_metadata", {
        metadata_id: "0",
        metadata: { media: "baz" },
      });
    },
    "Requires attached deposit of exactly 1 yoctoNEAR",
    "Updating NFT without yoctoNEAR deposit"
  );
  // bob cannot update at all
  await assertContractPanic(
    test,
    async () => {
      await bob.call(
        store,
        "update_metadata",
        {
          metadata_id: "0",
          metadata: { media: "baz" },
        },
        { attachedDeposit: "1" }
      );
    },
    "This method can only be called by the metadata creator",
    "Updating metadata by NFT owner"
  );
  // alice cannot lock without yocto deposit
  await assertContractPanic(
    test,
    async () => {
      await alice.call(store, "lock_metadata", {
        metadata_id: "0",
      });
    },
    "Requires attached deposit of exactly 1 yoctoNEAR",
    "Updating NFT without yoctoNEAR deposit"
  );
  // bob cannot lock at all
  await assertContractPanic(
    test,
    async () => {
      await bob.call(
        store,
        "lock_metadata",
        { metadata_id: "0" },
        { attachedDeposit: "1" }
      );
    },
    "This method can only be called by the metadata creator",
    "Updating metadata by NFT owner"
  );

  // lock metadata
  const lockMetadataCall = await alice.callRaw(
    store,
    "lock_metadata",
    { metadata_id: "0" },
    { attachedDeposit: "1" }
  );
  // assert event
  assertEventLogs(
    test,
    (lockMetadataCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "minting_metadata_update",
        data: {
          metadata_id: "0",
          minters_allowlist: null,
          price: null,
          is_dynamic: false,
        },
      },
    ],
    "locking metadata"
  );

  // assert that trying to update fails now
  await assertContractPanic(
    test,
    async () => {
      await alice.call(
        store,
        "lock_metadata",
        { metadata_id: "0" },
        { attachedDeposit: "1" }
      );
    },
    "Metadata is already locked",
    "Locking metadata twice"
  );
});

test("v2::minting_deposit", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;
  await createMetadata({
    alice,
    store,
    args: {
      metadata_id: "1",
      metadata: {},
      price: NEAR(0.01),
    },
  });

  // minting fails if no storage has been deposited
  await assertContractPanic(
    test,
    () =>
      bob.call(
        store,
        "mint_on_metadata",
        { metadata_id: "1", num_to_mint: 1, owner_id: bob.accountId },
        {
          attachedDeposit: NEAR(0.01),
        }
      ),
    "This mint requires a storage deposit of 5400000000000000000000 yoctoNEAR, you have 0",
    "minting without deposit"
  );

  // sponsored mints work
  await alice.call(
    store,
    "deposit_storage",
    { metadata_id: "1" },
    { attachedDeposit: NEAR(0.05) }
  );
  await bob.call(
    store,
    "mint_on_metadata",
    { metadata_id: "1", num_to_mint: 1, owner_id: bob.accountId },
    {
      attachedDeposit: NEAR(0.01),
    }
  );
});

// TODO: mint via FT
test("v2::ft_minting", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store, root } = test.context.accounts;
  const deployWnear = (name: string) =>
    createAndDeploy(root, name, {
      initialBalanceNear: "10",
      codePath: "../wasm/wnear.wasm",
      initMethod: "new",
      initArgs: {},
    });
  const wnear = await deployWnear("wnear");
  const wnear2 = await deployWnear("wnear2");
  const wrapNear = (account: NearAccount, wnear: NearAccount, amount: number) =>
    account.call(
      wnear,
      "near_deposit",
      {},
      { attachedDeposit: NEAR(amount).toString() }
    );
  await Promise.all([
    wrapNear(alice, wnear, 1),
    wrapNear(alice, wnear2, 1),
    wrapNear(bob, wnear, 1),
    wrapNear(bob, wnear2, 1),
    root.call(
      wnear,
      "storage_deposit",
      { account_id: store.accountId },
      { attachedDeposit: NEAR(0.1) }
    ),
    root.call(
      wnear2,
      "storage_deposit",
      { account_id: store.accountId },
      { attachedDeposit: NEAR(0.1) }
    ),
  ]);
  await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      price: NEAR(0.01),
      ft_contract_id: wnear.accountId,
    },
  });

  await bob.call(store, "deposit_storage", {}, { attachedDeposit: NEAR(0.1) });

  const getWnearBalance = async (account: NearAccount): Promise<BN> =>
    new BN(
      await wnear.view("ft_balance_of", { account_id: account.accountId })
    );
  const preAliceWnearBalance = await getWnearBalance(alice);

  const mintOnMetadataCall = await bob.callRaw(
    wnear,
    "ft_transfer_call",
    {
      receiver_id: store.accountId,
      amount: NEAR(0.05).toString(),
      msg: JSON.stringify({
        metadata_id: "0",
        num_to_mint: 3,
        owner_id: bob.accountId,
      }),
    },
    { attachedDeposit: "1", gas: Tgas(300) }
  );

  assertEventLogs(
    test,
    (mintOnMetadataCall as TransactionResult).logs.slice(1, 2),
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: bob.accountId,
            token_ids: ["0:0", "0:1", "0:2"],
            memo: '{"royalty":null,"split_owners":null,"meta_id":null,"meta_extra":null,"minter":"bob.test.near"}',
          },
        ],
      },
    ],
    "minting on metadata metadata"
  );

  // make sure alice got here wnear payout
  const postAliceWnearBalance = await getWnearBalance(alice);
  test.is(
    postAliceWnearBalance.sub(preAliceWnearBalance).toString(),
    NEAR(0.05).toString()
  );

  await assertContractPanic(
    test,
    async () => {
      await mintOnMetadata({
        bob,
        store,
        args: {
          metadata_id: "0",
          num_to_mint: 1,
          owner_id: bob.accountId,
        },
        deposit: 0.05,
      });
    },
    `This mint is required to be paid via FT: ${wnear.accountId}`,
    "Minting FT metadata with attached NEAR"
  );

  const wnear2MintCall = await bob.callRaw(
    wnear2,
    "ft_transfer_call",
    {
      receiver_id: store.accountId,
      amount: NEAR(0.05).toString(),
      msg: JSON.stringify({
        metadata_id: "0",
        num_to_mint: 3,
        owner_id: bob.accountId,
      }),
    },
    { attachedDeposit: "1", gas: Tgas(300) }
  );
  test.is(
    JSON.parse(wnear2MintCall.receiptFailureMessages[0]).ActionError.kind
      .FunctionCallError.ExecutionError,
    "Smart contract panicked: You need to use the correct FT to buy this token: wnear.test.near"
  );
});

test("v2::no_token_id_reuse", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, bob, store } = test.context.accounts;
  await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      price: NEAR(0.01),
    },
  });

  const mintOnMetadataCall = await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 1,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });

  assertEventLogs(
    test,
    (mintOnMetadataCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: bob.accountId,
            token_ids: ["0:0"],
            memo: '{"royalty":null,"split_owners":null,"meta_id":null,"meta_extra":null,"minter":"bob.test.near"}',
          },
        ],
      },
    ],
    "minting on metadata metadata"
  );

  await bob.call(
    store,
    "nft_batch_burn",
    { token_ids: ["0:0"] },
    { attachedDeposit: "1" }
  );

  const mintOnMetadataCall1 = await mintOnMetadata({
    bob,
    store,
    args: {
      metadata_id: "0",
      num_to_mint: 1,
      owner_id: bob.accountId,
    },
    deposit: 0.05,
  });
  assertEventLogs(
    test,
    (mintOnMetadataCall1 as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: bob.accountId,
            token_ids: ["0:1"],
            memo: '{"royalty":null,"split_owners":null,"meta_id":null,"meta_extra":null,"minter":"bob.test.near"}',
          },
        ],
      },
    ],
    "minting on metadata metadata"
  );
});
