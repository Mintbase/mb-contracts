import avaTest from "ava";
import { TransactionResult } from "near-workspaces";
import {
  assertEventLogs,
  failPromiseRejection,
  mintingDeposit,
  changeSettingsData,
  assertContractPanic,
} from "./utils/index.js";
import { setup, MB_VERSION, CHANGE_SETTING_VERSION } from "./setup.js";

const test = setup(avaTest);

test("v2::reset_splits", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, store } = test.context.accounts;

  const split_owners = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 6000;
    o["b.near"] = 4000;
    return o;
  })();

  await alice
    .call(
      store,
      "nft_batch_mint",
      {
        owner_id: alice.accountId,
        metadata: {},
        num_to_mint: 1,
        split_owners,
      },
      { attachedDeposit: mintingDeposit({ n_tokens: 1, n_splits: 2 }) }
    )
    .catch(failPromiseRejection(test, "minting"));

  const payout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "6000000000000000";
    p["b.near"] = "4000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: "0",
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

  await alice
    .call(
      store,
      "set_split_owners",
      {
        token_ids: ["0"],
        split_between: newSplitOwners,
      },
      { attachedDeposit: mintingDeposit({ n_tokens: 1, n_splits: 2 }) }
    )
    .catch(failPromiseRejection(test, "resetting splits"));

  // TODO: test logs

  const newPayout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "4000000000000000";
    p["b.near"] = "6000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: "0",
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

  // No minting cap exists initially
  test.is(await store.view("get_minting_cap"), null);

  // Setting minting cap works
  const setMintingCapCall = await alice.callRaw(
    store,
    "set_minting_cap",
    { minting_cap: 5 },
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
          set_minting_cap: "5",
        }),
      },
    ],
    "setting minting cap"
  );

  // New minting cap is successfuly returned
  test.is(await store.view("get_minting_cap"), 5);

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
      await alice.call(
        store,
        "nft_batch_mint",
        {
          owner_id: alice.accountId,
          metadata: {},
          num_to_mint: 20,
        },
        {
          attachedDeposit: mintingDeposit({ n_tokens: 20, metadata_bytes: 50 }),
        }
      );
    },
    "This mint would exceed the smart contracts minting cap",
    "Minting beyond cap"
  );
  // TODO: (low priority) cannot set set beyond already minted tokens
  // TODO: (low priority) requires yoctoNEAR deposit
});

test.skip("v2::open_minting", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, store } = test.context.accounts;

  // FIXME: not working?
  // No minting cap exists initially
  test.is(await store.view("get_open_minting"), false);

  // Setting minting cap works
  const allowOpenMintingCall = await alice.callRaw(
    store,
    "set_open_minting",
    { allow: true },
    { attachedDeposit: "1" }
  );

  assertEventLogs(
    test,
    (allowOpenMintingCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({
          // @ts-ignore
          allow_open_minting: true,
        }),
      },
    ],
    "setting minting cap"
  );

  // New minting cap is successfuly returned
  test.is(await store.view("get_open_minting"), true);

  // TODO: (medium priority) actually mint something
  // TODO: (low priority) disallow open minting and try to mint
});

test("v2::specify_token_ids_on_mint", async (test) => {
  if (MB_VERSION == "v1") {
    test.pass();
    return;
  }

  const { alice, store } = test.context.accounts;

  const mintCall = await alice.callRaw(
    store,
    "nft_batch_mint",
    {
      owner_id: alice.accountId,
      metadata: {},
      token_ids: ["12", "34", "56"],
    },
    {
      attachedDeposit: mintingDeposit({ n_tokens: 20, metadata_bytes: 50 }),
    }
  );

  assertEventLogs(
    test,
    (mintCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_mint",
        data: [
          {
            owner_id: alice.accountId,
            token_ids: ["12", "34", "56"],
            memo: JSON.stringify({
              royalty: null,
              split_owners: null,
              meta_id: null,
              meta_extra: null,
              minter: alice.accountId,
            }),
          },
        ],
      },
    ],
    "specifying token IDs when minting"
  );
});
