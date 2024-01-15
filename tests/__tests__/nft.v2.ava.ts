import avaTest from "ava";
import { NearAccount, TransactionResult } from "near-workspaces";
import {
  assertEventLogs,
  failPromiseRejection,
  mintingDeposit,
  changeSettingsData,
  assertContractPanic,
  NEAR,
} from "./utils/index.js";
import { setup, MB_VERSION, CHANGE_SETTING_VERSION } from "./setup.js";

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
  const call = await bob.callRaw(store, "mint_on_metadata", args, {
    attachedDeposit: NEAR(deposit),
  });
  if (call.failed) throw new Error(JSON.stringify(call));
  return call;
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
  // TODO: (low priority) cannot set set beyond already minted tokens
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
          metadata_id: 0,
          minters_allowlist: null,
          price: NEAR(0.01).toString(),
        },
      },
    ],
    "creating metadata"
  );

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
          metadata_id: 12,
          minters_allowlist: null,
          price: NEAR(0.01).toString(),
        },
      },
    ],
    "creating metadata"
  );

  const createMetadataCall2 = await createMetadata({
    alice,
    store,
    args: {
      metadata: {},
      royalty_args: {
        split_between: (() => {
          const s: Record<string, number> = {};
          s[alice.accountId] = 6000;
          s[bob.accountId] = 4000;
          return s;
        })(),
        percentage: 2000,
      },
      price: NEAR(0.01),
    },
  });
  assertEventLogs(
    test,
    (createMetadataCall2 as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "2.0.0",
        event: "create_metadata",
        data: {
          creator: alice.accountId,
          metadata_id: 1,
          minters_allowlist: null,
          price: NEAR(0.01).toString(),
        },
      },
    ],
    "creating metadata"
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
    "Attached deposit must cover storage usage, token price and minting fee",
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
          metadata_id: 0,
          minters_allowlist: [bob.accountId],
          price: NEAR(0.01).toString(),
        },
      },
    ],
    "creating metadata"
  );

  // bob can mint
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
    `${alice.accountId} is not allowed to mint this metadata`,
    "Minting token ID twice"
  );
});
