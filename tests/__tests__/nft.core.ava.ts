import { TransactionResult } from "near-workspaces";
import avaTest from "ava";
import {
  assertContractPanics,
  DEPLOY_STORE_RENT,
  DEPLOY_STORE_GAS,
  assertContractTokenOwners,
  assertEventLogs,
  failPromiseRejection,
  Tgas,
  mintingDeposit,
  batchMint,
  parseEvent,
} from "./utils/index.js";
import { MB_VERSION, setup } from "./setup.js";

const test = setup(avaTest);

test("core", async (test) => {
  const { factory, store, alice, bob, carol } = test.context.accounts;

  // store creation
  await bob
    .call(
      factory,
      "create_store",
      {
        owner_id: alice.accountId,
        metadata: {
          spec: "nft-1.0.0",
          name: "bob",
          symbol: "BOB",
        },
      },
      { attachedDeposit: DEPLOY_STORE_RENT, gas: DEPLOY_STORE_GAS }
    )
    .catch(failPromiseRejection(test, "creating store"));
  // const store = root.getAccount(`bob.${factory.accountId}`);
  // TODO::testing::medium: check event logs

  // TODO::testing::medium trying deployment with forbidden names
  //  - reserved names: "market", "loan"
  //  - taken names, in this case "alice"

  // minting
  // const mintCall = await alice
  //   .callRaw(
  //     store,
  //     "nft_batch_mint",
  //     { owner_id: alice.accountId, metadata: {}, num_to_mint: 6 },
  //     {
  //       attachedDeposit: mintingDeposit({
  //         n_tokens: 6,
  //         n_royalties: 0,
  //         n_splits: 0,
  //       }),
  //     }
  //   )
  //   .catch(failPromiseRejection(test, "minting"));
  const mintCall = await batchMint({ owner: alice, store, num_to_mint: 6 });
  const tokenIds = parseEvent((mintCall as TransactionResult).logs[0]).data[0]
    .token_ids as string[];

  // check minting logs
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
            owner_id: "alice.test.near",
            token_ids: tokenIds,
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
    "minting"
  );

  // inspecting minted tokens (implicitly tests `nft_token`)
  await assertContractTokenOwners(
    { test, store },
    tokenIds.map((id) => ({ token_id: id, owner_id: alice.accountId })),
    "After minting"
  ).catch(failPromiseRejection(test, "checking token format"));
  // test.pass();
  // return;

  if (MB_VERSION == "v1") {
    await assertContractPanics(test, [
      // try to mint while not being minter
      [
        async () => {
          await bob.call(
            store,
            "nft_batch_mint",
            { owner_id: bob.accountId, metadata: {}, num_to_mint: 1 },
            { attachedDeposit: "1" }
          );
        },
        `${bob.accountId} is not allowed to mint on this store`,
        "Bob tried minting without minter permission",
      ],
      // try minting without yoctoNEAR deposit
      [
        async () => {
          await alice.call(store, "nft_batch_mint", {
            owner_id: alice.accountId,
            metadata: {},
            num_to_mint: 1,
          });
        },
        "Requires deposit of at least 1 yoctoNEAR",
        "Alice tried minting without yoctoNEAR deposit",
      ],
    ]);
  }

  // transfering a single token
  const transferCall = await alice
    .callRaw(
      store,
      "nft_transfer",
      { receiver_id: bob.accountId, token_id: tokenIds[0] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection(test, "transferring"));

  // check transfer logs
  assertEventLogs(
    test,
    (transferCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_transfer",
        data: [
          {
            authorized_id: null,
            old_owner_id: "alice.test.near",
            new_owner_id: "bob.test.near",
            token_ids: [tokenIds[0]],
            memo: null,
          },
        ],
      },
    ],
    "transferring"
  );

  await assertContractPanics(test, [
    // try to transfer unowned token (random bob)
    [
      async () => {
        await bob.call(
          store,
          "nft_transfer",
          { receiver_id: bob.accountId, token_id: tokenIds[1] },
          { attachedDeposit: "1" }
        );
      },
      "Disallowing approvals without approval ID",
      "Bob tried to transfer an unowned token",
    ],
    // try to transfer unowned token (store owner)
    [
      async () => {
        await alice.call(
          store,
          "nft_transfer",
          { receiver_id: alice.accountId, token_id: tokenIds[0] },
          { attachedDeposit: "1" }
        );
      },
      "Disallowing approvals without approval ID",
      "Alice tried to transfer an unowned token",
    ],
  ]);

  // batch transfering tokens
  const batchTransferCall = await alice
    .callRaw(
      store,
      "nft_batch_transfer",
      // TODO::contracts::low: undescriptive param name
      // TODO::contracts::low: why is this a tuple whereas `nft_transfer` is
      //  a record?
      // TODO::contracts::low: missing memo parameter?
      {
        token_ids: [
          [tokenIds[1], bob.accountId],
          [tokenIds[2], carol.accountId],
        ],
      },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection(test, "batch transferring"));

  // check transfer logs
  // TODO::contracts::low: should empty fields be serialized as null or
  //  simply omitted? -> null might make sense for the indexer
  // TODO::testing::low: assert event when batch transferring two to the same
  // address
  assertEventLogs(
    test,
    (batchTransferCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_transfer",
        data: [
          {
            authorized_id: null,
            old_owner_id: "alice.test.near",
            new_owner_id: "bob.test.near",
            token_ids: [tokenIds[1]],
            memo: null,
          },
          {
            authorized_id: null,
            old_owner_id: "alice.test.near",
            new_owner_id: "carol.test.near",
            token_ids: [tokenIds[2]],
            memo: null,
          },
        ],
      },
    ],
    "batch transferring"
  );

  await assertContractPanics(test, [
    // try to batch transfer unowned tokens (random bob)
    [
      async () => {
        await bob.call(
          store,
          "nft_batch_transfer",
          {
            token_ids: [
              [tokenIds[1], carol.accountId],
              [tokenIds[2], bob.accountId],
            ],
          },
          { attachedDeposit: "1" }
        );
      },
      `${bob.accountId} is required to own token 2`,
      "Bob tried to batch transfer unowned tokens",
    ],
    // try to batch transfer unowned tokens (store owner)
    [
      async () => {
        await alice.call(
          store,
          "nft_batch_transfer",
          {
            token_ids: [
              [tokenIds[0], alice.accountId],
              [tokenIds[1], alice.accountId],
            ],
          },
          { attachedDeposit: "1" }
        );
      },
      `${alice.accountId} is required to own token 0`,
      "Alice tried to batch transfer unowned tokens",
    ],
    // try to batch transfer without yoctoNEAR deposit
    [
      async () => {
        await alice.call(store, "nft_batch_transfer", {
          token_ids: [
            [tokenIds[0], alice.accountId],
            [tokenIds[1], alice.accountId],
          ],
        });
      },
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Alice tried to batch transfer tokens without yoctoNEAR deposit",
    ],
  ]);

  // checking token ownership
  await assertContractTokenOwners(
    { test, store },
    [
      { token_id: tokenIds[0], owner_id: bob.accountId },
      { token_id: tokenIds[1], owner_id: bob.accountId },
      { token_id: tokenIds[2], owner_id: carol.accountId },
      { token_id: tokenIds[3], owner_id: alice.accountId },
      { token_id: tokenIds[4], owner_id: alice.accountId },
      { token_id: tokenIds[5], owner_id: alice.accountId },
    ],
    "After transfers"
  ).catch(failPromiseRejection(test, "checking token ownership"));

  // burning tokens
  const burnCall = await alice
    .callRaw(
      store,
      "nft_batch_burn",
      { token_ids: [tokenIds[4], tokenIds[5]] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection(test, "burning"));

  // check burn logs
  assertEventLogs(
    test,
    (burnCall as TransactionResult).logs,
    [
      {
        standard: "nep171",
        version: "1.0.0",
        event: "nft_burn",
        data: [
          {
            owner_id: "alice.test.near",
            authorized_id: null,
            token_ids: [tokenIds[4], tokenIds[5]],
            memo: null,
          },
        ],
      },
    ],
    "burning"
  );

  await assertContractPanics(test, [
    // try to burn unowned tokens (random bob)
    [
      async () => {
        await bob.call(
          store,
          "nft_batch_burn",
          { token_ids: [tokenIds[1], tokenIds[2]] },
          { attachedDeposit: "1" }
        );
      },
      `${bob.accountId} is required to own token 2`,
      "Bob tried to burn unowned tokens",
    ],
    // try to burn unowned tokens (store owner)
    [
      async () => {
        await alice.call(
          store,
          "nft_batch_burn",
          { token_ids: [tokenIds[0]] },
          { attachedDeposit: "1" }
        );
      },
      `${alice.accountId} is required to own token 0`,
      "Alice tried to burn unowned tokens",
    ],
    // try to burn tokens without deposit
    [
      async () => {
        await alice.call(store, "nft_batch_burn", {
          token_ids: [tokenIds[3]],
        });
      },
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Alice tried to burn tokens without yoctoNEAR deposit",
    ],
    // TODO: figure out if alice is still token owner
    // TODO::testing::medium: can no longer transfer burned token
    // TODO::testing::medium: cannot burn token twice
  ]);

  // TODO::testing::low: transfer store ownership
  // TODO::testing::low: try to transfer store ownership (random bob)
  // TODO::testing::low: try to transfer store ownership without yN deposit

  // TODO::testing::low: try to undeploy contract (random bob)
  // TODO::testing::low: undeploy contract (store owner)
});

test("batch-mint", async (test) => {
  if (MB_VERSION == "v2") {
    test.pass();
    return;
  }

  const { alice, store } = test.context.accounts;

  const mintCall = await alice.callRaw(
    store,
    "nft_batch_mint",
    {
      owner_id: alice.accountId,
      metadata: {
        reference: "x".repeat(43), // 43 chars hash, would be 63 with arweave base URI included
        media: "x".repeat(43),
        starts_at: "1672531200000000000",
        expires_at: "1672531200000000000",
      },
      num_to_mint: 125,
    },
    {
      attachedDeposit: mintingDeposit({
        n_tokens: 125,
        metadata_bytes: 110000,
      }),
      gas: Tgas(225),
    }
  );

  // @ts-ignore
  test.is(typeof mintCall.status.SuccessValue, "string");
});
