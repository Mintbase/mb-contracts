import { TransactionResult } from "near-workspaces";
import avaTest from "ava";
import {
  assertApprovals,
  assertNoApprovals,
  assertContractPanics,
  assertEventLogs,
  batchMint,
  mNEAR,
  getBalance,
  assertContractTokenOwners,
  assertNoApproval,
  changeSettingsData,
  getTokenIds,
} from "./utils/index.js";
import { setup, CHANGE_SETTING_VERSION, MB_VERSION } from "./setup.js";

const test = setup(avaTest);

test("approvals::core", async (test) => {
  const { alice, bob, carol, store } = test.context.accounts;

  const failPromiseRejection = (msg: string) => (e: any) => {
    test.log(`Promise rejected while ${msg}:`);
    test.log(e);
    test.fail();
  };

  const mintCall = await batchMint({
    owner: alice,
    store,
    num_to_mint: 4,
  }).catch(failPromiseRejection("minting"));
  const tokenIds = getTokenIds(mintCall as TransactionResult);
  // // assert correctness of current owners
  // await assertContractTokenOwners(
  //   test,
  //   store,
  //   [
  //     { id: "0", owner_id: alice.accountId },
  //     { id: "1", owner_id: alice.accountId },
  //     { id: "2", owner_id: alice.accountId },
  //     { id: "3", owner_id: alice.accountId },
  //   ],
  //   "minting"
  // );

  // assert correctness of current approvals
  await assertNoApprovals(
    { test, store },
    [
      { token_id: tokenIds[0], approved_account_id: bob.accountId },
      { token_id: tokenIds[1], approved_account_id: bob.accountId },
      { token_id: tokenIds[2], approved_account_id: bob.accountId },
      { token_id: tokenIds[3], approved_account_id: bob.accountId },
    ],
    "minting"
  );

  // -------------------------------- approve --------------------------------
  const approveCall = await alice
    .callRaw(
      store,
      "nft_approve",
      { token_id: tokenIds[0], account_id: bob.accountId },
      { attachedDeposit: mNEAR(0.8) }
    )
    .catch(failPromiseRejection("approving"));
  // check event logs
  assertEventLogs(
    test,
    (approveCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "0.1.0",
        event: "nft_approve",
        data: [
          { token_id: tokenIds[0], approval_id: 0, account_id: bob.accountId },
        ],
      },
    ],
    "approving"
  );

  await assertContractPanics(test, [
    // try approving when not owning token
    [
      async () =>
        bob.call(
          store,
          "nft_approve",
          { token_id: tokenIds[1], account_id: bob.accountId },
          { attachedDeposit: mNEAR(0.8) }
        ),
      `${bob.accountId} is required to own token 1`,
      "Bob tried approving on unowned token",
    ],
    // require at least one yoctoNEAR to approve
    [
      async () =>
        alice.call(
          store,
          "nft_approve",
          { token_id: tokenIds[1], account_id: bob.accountId },
          { attachedDeposit: mNEAR(0.79) }
        ),
      "Requires storage deposit of at least 800000000000000000000 yoctoNEAR",
      "Alice tried approving with insufficient deposit",
    ],
  ]);

  // assert correctness of current approvals
  await assertApprovals(
    { test, store },
    [
      {
        token_id: tokenIds[0],
        approved_account_id: bob.accountId,
        approval_id: 0,
      },
    ],
    "approving"
  );
  await assertNoApprovals(
    { test, store },
    [
      { token_id: tokenIds[1], approved_account_id: bob.accountId },
      { token_id: tokenIds[2], approved_account_id: bob.accountId },
      { token_id: tokenIds[3], approved_account_id: bob.accountId },
    ],
    "approving"
  );
  test.is(
    await store.view("nft_approval_id", {
      token_id: tokenIds[0],
      account_id: bob.accountId,
    }),
    0
  );

  // ----------------------------- batch approve -----------------------------
  const batchApproveCall = await alice
    .callRaw(
      store,
      "nft_batch_approve",
      { token_ids: [tokenIds[1], tokenIds[2]], account_id: bob.accountId },
      { attachedDeposit: mNEAR(1.6) } // no value for this in mintbase-js
    )
    .catch(failPromiseRejection("batch approving"));
  // check event logs
  assertEventLogs(
    test,
    (batchApproveCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "0.1.0",
        event: "nft_approve",
        data: [
          { token_id: tokenIds[1], approval_id: 1, account_id: bob.accountId },
          { token_id: tokenIds[2], approval_id: 2, account_id: bob.accountId },
        ],
      },
    ],
    "batch approving"
  );

  await assertContractPanics(test, [
    // try batch approving when not owning token
    [
      async () =>
        bob.call(
          store,
          "nft_batch_approve",
          { token_ids: [tokenIds[2], tokenIds[3]], account_id: bob.accountId },
          { attachedDeposit: mNEAR(1.6) }
        ),
      `${bob.accountId} is required to own token 2`,
      "Bob tried batch approving on unowned tokens",
    ],
    // require at sufficient deposit to cover storage rent
    [
      async () =>
        alice.call(
          store,
          "nft_batch_approve",
          { token_ids: [tokenIds[3]], account_id: bob.accountId },
          { attachedDeposit: mNEAR(0.79) }
        ),
      "Requires storage deposit of at least 800000000000000000000 yoctoNEAR",
      "Alice tried batch approving with insufficient deposit",
    ],
  ]);

  // assert correctness of current approvals
  await assertApprovals(
    { test, store },
    [
      {
        token_id: tokenIds[0],
        approved_account_id: bob.accountId,
        approval_id: 0,
      },
      {
        token_id: tokenIds[1],
        approved_account_id: bob.accountId,
        approval_id: 1,
      },
      {
        token_id: tokenIds[2],
        approved_account_id: bob.accountId,
        approval_id: 2,
      },
    ],
    "batch approving"
  );
  await assertNoApprovals(
    { test, store },
    [{ token_id: tokenIds[3], approved_account_id: bob.accountId }],
    "batch approving"
  );

  // -------------------------------- revoke ---------------------------------
  // get bob's balance to check the refunding
  const aliceBalance1 = await getBalance(alice);
  const revokeCall = await alice
    .callRaw(
      store,
      "nft_revoke",
      {
        token_id: tokenIds[2],
        account_id: bob.accountId,
      },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("revoking"));
  // const aliceBalance2 = await getBalance(alice);
  // const balanceDiff = aliceBalance1.sub(aliceBalance2);
  // const gas = (revokeCall as TransactionResult).gas_burnt;
  // const nearGasBN = new BN(gas.toString()).mul(new BN(100e6)).toString();
  // const nearGas = new ava.NEAR(nearGasBN);
  // test.log(`Alice's balance before revoking: ${aliceBalance1.toHuman()}`);
  // test.log(`Alice's balance after revoking:  ${aliceBalance2.toHuman()}`);
  // test.log(`Difference:                      ${balanceDiff.toHuman()}`);
  // test.log(`Gas costs (1 Tgas = 0.3 mNEAR):  ${nearGas.toHuman()}`);
  // test.log(`Gas costs (gas units):           ${gas.toHuman()}`);
  // test.fail();

  // check event logs
  assertEventLogs(
    test,
    (revokeCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "0.1.0",
        event: "nft_revoke",
        // TODO::store::low: for `nft_approve`, data is an array, here
        //  it's an object -> should have the same predictable structure
        data: { token_id: tokenIds[2], account_id: bob.accountId },
      },
    ],
    "revoking"
  );
  // check if revoking refunds the storage deposit
  // TODO::idk::medium: 6 mNEAR gone missing -> create issue on github
  // await assertBalanceChange(
  //   test,
  //   {
  //     account: alice,
  //     // subtract the yoctoNEAR deposit
  //     ref: aliceBalance1.sub(new BN("1")),
  //     diff: mNEAR(0.8),
  //     gas: (revokeCall as TransactionResult).gas_burnt,
  //   },
  //   "Revoking"
  // );

  await assertContractPanics(test, [
    // try revoking when not owning token
    [
      async () =>
        bob.call(
          store,
          "nft_revoke",
          {
            token_id: tokenIds[1],
            account_id: bob.accountId,
          },
          { attachedDeposit: "1" }
        ),
      `${bob.accountId} is required to own token 1`,
      "Bob tried revoking on unowned token",
    ],
    // require at least one yoctoNEAR to revoke
    [
      async () =>
        alice.call(store, "nft_revoke", {
          token_id: tokenIds[0],
          account_id: bob.accountId,
        }),
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Alice tried revoking without yoctoNEAR deposit",
    ],
  ]);

  // assert correctness of current approvals
  await assertApprovals(
    { test, store },
    [
      {
        token_id: tokenIds[0],
        approved_account_id: bob.accountId,
        approval_id: 0,
      },
      {
        token_id: tokenIds[1],
        approved_account_id: bob.accountId,
        approval_id: 1,
      },
    ],
    "revoking"
  );
  await assertNoApprovals(
    { test, store },
    [
      { token_id: tokenIds[2], approved_account_id: bob.accountId },
      { token_id: tokenIds[3], approved_account_id: bob.accountId },
    ],
    "revoking"
  );

  // ------------------------------ revoke_all -------------------------------
  // prior to revoking all, we need a token with two approvals
  await alice.call(
    store,
    "nft_batch_approve",
    { token_ids: [tokenIds[0], tokenIds[1]], account_id: carol.accountId },
    { attachedDeposit: mNEAR(1.61) } // no value for this in mintbase-js
  );
  await assertApprovals(
    { test, store },
    [
      {
        token_id: tokenIds[0],
        approved_account_id: carol.accountId,
        approval_id: 3,
      },
      {
        token_id: tokenIds[1],
        approved_account_id: carol.accountId,
        approval_id: 4,
      },
    ],
    "preparing revoke_all"
  );

  // actual call
  // const aliceBalance2 = await getBalance(alice);
  const revokeAllCall = await alice
    .callRaw(
      store,
      "nft_revoke_all",
      { token_id: tokenIds[1] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("revoking all"));
  // check event logs
  assertEventLogs(
    test,
    (revokeAllCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: "0.1.0",
        event: "nft_revoke_all",
        data: { token_id: tokenIds[1] },
      },
    ],
    "revoking all"
  );
  // // check if revoking all refunds the required security deposit
  // // FIXME::testing::low: this cannot test properly because the cost is so low
  // // -> use TransactionResult::gas_burnt()
  // await assertBalanceChange(
  //   test,
  //   { account: alice, ref: aliceBalance2, diff: mNEAR(1.6) },
  //   "Revoking all"
  // );

  await assertContractPanics(test, [
    // try revoking all when not owning token
    [
      async () =>
        bob.call(
          store,
          "nft_revoke_all",
          { token_id: tokenIds[0] },
          { attachedDeposit: "1" }
        ),
      `${bob.accountId} is required to own token 0`,
      "Bob tried revoking all on unowned token",
    ],
    // require at least one yoctoNEAR to revoke all
    [
      async () =>
        alice.call(store, "nft_revoke_all", { token_id: tokenIds[0] }),
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Alice tried revoking all without yoctoNEAR deposit",
    ],
  ]);

  // // assert correctness of current approvals
  await assertApprovals(
    { test, store },
    [
      {
        token_id: tokenIds[0],
        approved_account_id: bob.accountId,
        approval_id: 0,
      },
      {
        token_id: tokenIds[0],
        approved_account_id: carol.accountId,
        approval_id: 3,
      },
    ],
    "revoking all"
  );
  await assertNoApprovals(
    { test, store },
    [
      { token_id: tokenIds[1], approved_account_id: carol.accountId },
      { token_id: tokenIds[1], approved_account_id: bob.accountId },
      { token_id: tokenIds[2], approved_account_id: bob.accountId },
      { token_id: tokenIds[3], approved_account_id: bob.accountId },
    ],
    "revoking all"
  );
});

test("approvals::minting", async (test) => {
  const { alice, bob, carol, dave, store } = test.context.accounts;
  const failPromiseRejection = (msg: string) => (e: any) => {
    test.log(`Promise rejected while ${msg}:`);
    test.log(e);
    test.fail();
  };
  const CHANGE_MINTERS_METHOD =
    MB_VERSION === "v1" ? "batch_change_minters" : "batch_change_creators";
  const CHECK_MINTERS_METHOD =
    MB_VERSION === "v1" ? "check_is_minter" : "check_is_creator";
  const LIST_MINTERS_METHOD =
    MB_VERSION === "v1" ? "list_minters" : "list_creators";

  // ---------------------------- authorized mint ----------------------------
  // TODO::store::low: this increases storage, shouldn't it then require
  //  a sufficient deposit? -> this is not third party-territory, only the
  //  owner can call this
  const grantMinterCall = await alice
    .callRaw(
      store,
      CHANGE_MINTERS_METHOD,
      { grant: [bob.accountId] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("grant minting rights"));
  // check logs
  assertEventLogs(
    test,
    (grantMinterCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ granted_minter: bob.accountId }),
      },
    ],
    "grant minting rights"
  );

  await assertContractPanics(test, [
    // only owner can grant minting rights
    [
      async () =>
        bob.call(
          store,
          CHANGE_MINTERS_METHOD,
          { grant: [bob.accountId] },
          { attachedDeposit: "1" }
        ),
      "This method can only be called by the store owner",
      "Bob tried granting himself minting rights",
    ],
    //  require deposit
    [
      async () =>
        alice.call(store, CHANGE_MINTERS_METHOD, { grant: [bob.accountId] }),
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Alice tried to grant minting rights without yoctoNEAR deposit",
    ],
  ]);
  // check contract state (implicitly tests `check_is_minter`)
  test.true(
    await store.view(CHECK_MINTERS_METHOD, { account_id: bob.accountId }),
    "Failed to grant minting rights to Bob"
  );
  test.false(
    await store.view(CHECK_MINTERS_METHOD, { account_id: carol.accountId }),
    "How on earth did Carol get minting rights?"
  );
  // checking the list_minters method
  test.deepEqual(
    await store.view(LIST_MINTERS_METHOD),
    [alice.accountId, bob.accountId],
    "Bad minters list after granting minting rigths to Bob"
  );

  // actual minting
  // TODO::store::low: shouldn't third party minting require deposits to
  //  cover storage costs? -> otherwise third-party minters might exhaust a
  //  contracts storage
  const mintCall = await batchMint({
    owner: bob,
    store,
    num_to_mint: 2,
  });
  const tokenIds = getTokenIds(mintCall);

  // check logs
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
            owner_id: bob.accountId,
            token_ids: tokenIds,
            memo: JSON.stringify({
              royalty: null,
              split_owners: null,
              meta_id: null,
              meta_extra: null,
              minter: bob.accountId,
            }),
          },
        ],
      },
    ],
    "approved minting"
  );

  // check contract state
  assertContractTokenOwners(
    { test, store },
    [
      { token_id: tokenIds[0], owner_id: bob.accountId },
      { token_id: tokenIds[1], owner_id: bob.accountId },
    ],
    "approved minting"
  );

  // revoke minting rights
  const revokeMinterCall = await alice
    .callRaw(
      store,
      CHANGE_MINTERS_METHOD,
      { revoke: [bob.accountId] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("revoke minting rights"));

  // check logs
  assertEventLogs(
    test,
    (revokeMinterCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ revoked_minter: bob.accountId }),
      },
    ],
    "approved minting"
  );

  await assertContractPanics(test, [
    // requires yoctoNEAR deposit
    [
      async () =>
        alice.call(store, CHANGE_MINTERS_METHOD, { revoke: [bob.accountId] }),
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Alice tried to revoke minting rights without yoctoNEAR deposit",
    ],
    // owner cannot revoke their own minting rights
    [
      async () =>
        alice.call(
          store,
          CHANGE_MINTERS_METHOD,
          { revoke: [alice.accountId] },
          { attachedDeposit: "1" }
        ),
      "Owner cannot be removed from minters",
      "Alice tried to revoke her own minting rights",
    ],
  ]);

  // check contract state
  test.false(
    await store.view(CHECK_MINTERS_METHOD, { account_id: bob.accountId }),
    "Failed to revoke Bob's minting rights"
  );
  // checking the list_minters method
  test.deepEqual(
    await store.view(LIST_MINTERS_METHOD),
    [alice.accountId],
    "Bad minters list after granting minting rights to Bob"
  );

  // batch_change_minters: add bob and carol
  const batchGrantMinterCall = await alice
    .callRaw(
      store,
      CHANGE_MINTERS_METHOD,
      { grant: [bob.accountId, carol.accountId] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("batch grant minter rights"));

  // check logs
  assertEventLogs(
    test,
    (batchGrantMinterCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ granted_minter: bob.accountId }),
      },
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ granted_minter: carol.accountId }),
      },
    ],
    "batch grant minter rights"
  );
  test.deepEqual(
    await store.view(LIST_MINTERS_METHOD),
    [alice.accountId, bob.accountId, carol.accountId],
    "Bad minters list after batch granting minter rights"
  );

  // batch_change_minters: change carol to dave
  const batchChangeMinterCall = await alice
    .callRaw(
      store,
      CHANGE_MINTERS_METHOD,
      { revoke: [carol.accountId], grant: [dave.accountId] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("batch change minter rights"));
  // check logs
  assertEventLogs(
    test,
    (batchChangeMinterCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ granted_minter: dave.accountId }),
      },
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ revoked_minter: carol.accountId }),
      },
    ],
    "batch change minter rights"
  );
  test.deepEqual(
    await store.view(LIST_MINTERS_METHOD),
    [alice.accountId, bob.accountId, dave.accountId],
    "Bad minters list after batch changing minter rights"
  );

  // batch_change_minters: revoke bob and dave
  const batchRevokeMinterCall = await alice
    .callRaw(
      store,
      CHANGE_MINTERS_METHOD,
      { revoke: [bob.accountId, dave.accountId] },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("batch revoke minter rights"));
  // check logs
  assertEventLogs(
    test,
    (batchRevokeMinterCall as TransactionResult).logs,
    [
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ revoked_minter: bob.accountId }),
      },
      {
        standard: "mb_store",
        version: CHANGE_SETTING_VERSION,
        event: "change_setting",
        data: changeSettingsData({ revoked_minter: dave.accountId }),
      },
    ],
    "batch revoke minter rights"
  );
  test.deepEqual(
    await store.view(LIST_MINTERS_METHOD),
    [alice.accountId],
    "Bad minters list after batch revoking minter rights"
  );
});

test("approvals::token-actions", async (test) => {
  const { alice, bob, carol, store } = test.context.accounts;

  const failPromiseRejection = (msg: string) => (e: any) => {
    test.log(`Promise rejected while ${msg}:`);
    test.log(e);
    test.fail();
  };

  const mintCall = await batchMint({
    owner: alice,
    store,
    num_to_mint: 5,
  }).catch(failPromiseRejection("minting"));
  const tokenIds = getTokenIds(mintCall as TransactionResult);

  await alice
    .call(
      store,
      "nft_batch_approve",
      {
        token_ids: tokenIds,
        account_id: bob.accountId,
      },
      { attachedDeposit: mNEAR(4) } // no value for this in mintbase-js
    )
    .catch(failPromiseRejection("approving"));

  // -------------------------- authorized transfer --------------------------
  const transferCall = await bob
    .callRaw(
      store,
      "nft_transfer",
      { receiver_id: carol.accountId, token_id: tokenIds[0], approval_id: 0 },
      { attachedDeposit: "1" }
    )
    .catch(failPromiseRejection("transferring (approved)"));
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
            authorized_id: bob.accountId,
            old_owner_id: alice.accountId,
            new_owner_id: carol.accountId,
            token_ids: [tokenIds[0]],
            memo: null,
          },
        ],
      },
    ],
    "transferring (approved)"
  );

  await assertContractPanics(test, [
    // try transferring without approval ID
    [
      async () => {
        await bob.call(
          store,
          "nft_transfer",
          { receiver_id: carol.accountId, token_id: tokenIds[1] },
          { attachedDeposit: "1" }
        );
      },
      "Disallowing approvals without approval ID",
      "Bob tried transferring (approved) without approval_id",
    ],
    // require at least one yoctoNEAR to transfer
    [
      async () => {
        await bob.call(store, "nft_transfer", {
          receiver_id: carol.accountId,
          token_id: tokenIds[1],
          approval_id: 1,
        });
      },
      "Requires attached deposit of exactly 1 yoctoNEAR",
      "Bob tried transferring (approved) without yoctoNEAR deposit",
    ],
    // TODO::testing::medium workaround until fixed for not being able to
    //  check absence of approval
    [
      async () => {
        await bob.call(
          store,
          "nft_transfer",
          {
            receiver_id: carol.accountId,
            token_id: tokenIds[0],
            approval_id: 0,
          },
          { attachedDeposit: "1" }
        );
      },
      `${bob.accountId} has no approval for token 0`,
      "Bob tried transferring without having approval",
    ],
  ]);

  // token must now belong to carol
  await assertContractTokenOwners(
    { test, store },
    [
      { token_id: tokenIds[0], owner_id: carol.accountId },
      { token_id: tokenIds[1], owner_id: alice.accountId },
      { token_id: tokenIds[2], owner_id: alice.accountId },
      { token_id: tokenIds[3], owner_id: alice.accountId },
    ],
    "Bad ownership state after approved transfer"
  );
  // approval must have cleared -> FIXME: cannot check properly, because API is broken
  assertNoApproval(
    { test, store },
    { token_id: tokenIds[1], approved_account_id: bob.accountId },
    "Bob didn't loose approval after transfer"
  );
});

// only run this test when you change something about it, it takes forever
test.skip("approvals::capping", async (test) => {
  test.timeout(300000); // 5 minutes
  const { alice, bob, carol, store } = test.context.accounts;

  const failPromiseRejection = (msg: string) => (e: any) => {
    test.log(`Promise rejected while ${msg}:`);
    test.log(e);
    test.fail();
  };
  const mintCall = await batchMint({
    owner: alice,
    store,
    num_to_mint: 1,
  }).catch(failPromiseRejection("minting"));
  const token_id = getTokenIds(mintCall as TransactionResult)[0];

  const approved_account_ids: Record<string, number> = {};
  for (let i = 0; i < 100; i++) {
    const account_id = `account${i}.near`;
    approved_account_ids[account_id] = i;
    await alice
      .call(
        store,
        "nft_approve",
        { token_id, account_id },
        { attachedDeposit: mNEAR(0.8).toString() }
      )
      .catch(failPromiseRejection(`approving ${i}`));
  }

  test.deepEqual(
    ((await store.view("nft_token", { token_id })) as any).approved_account_ids,
    approved_account_ids
  );

  await alice
    .call(
      store,
      "nft_revoke_all",
      { token_id },
      {
        attachedDeposit: "1",
        gas: "5000000000000",
      }
    )
    .catch(failPromiseRejection("revoking all"));

  test.deepEqual(
    ((await store.view("nft_token", { token_id })) as any).approved_account_ids,
    {}
  );
});
