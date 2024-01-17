import avaTest from "ava";
import { assertTokensAre, batchMint, parseEvent } from "./utils/index.js";
import { setup } from "./setup.js";
import { TransactionResult } from "near-workspaces";

const test = setup(avaTest);

test("enumeration", async (test) => {
  const { alice, bob, store } = test.context.accounts;

  const failPromiseRejection = (msg: string) => (e: any) => {
    test.log(`Promise rejected while ${msg}:`);
    test.log(e);
    test.fail();
  };

  // seeding: mint 4 tokens (2 for Alice, 2 for Bob)
  const aliceMintCall = await batchMint({
    owner: alice,
    store,
    num_to_mint: 2,
  }).catch(failPromiseRejection("minting"));
  const bobMintCall = await batchMint({
    owner: alice,
    store,
    num_to_mint: 2,
    owner_id: bob.accountId,
  }).catch(failPromiseRejection("minting"));
  const aliceTokenIds = parseEvent((aliceMintCall as TransactionResult).logs[0])
    .data[0].token_ids as string[];
  const bobTokenIds = parseEvent((bobMintCall as TransactionResult).logs[0])
    .data[0].token_ids as string[];

  // testing `nft_total_supply` and `nft_supply_for_owner`
  test.is(await store.view("nft_total_supply", {}), "4");
  test.is(
    await store.view("nft_supply_for_owner", { account_id: alice.accountId }),
    "2"
  );
  test.is(
    await store.view("nft_supply_for_owner", { account_id: bob.accountId }),
    "2"
  );

  // call `nft_tokens` without params
  assertTokensAre(
    test,
    await store.view("nft_tokens", {}),
    [
      ...aliceTokenIds.map((id) => ({
        token_id: id,
        owner_id: alice.accountId,
      })),
      ...bobTokenIds.map((id) => ({ token_id: id, owner_id: bob.accountId })),
    ],
    "`nft_tokens({})` output is wrong"
  );

  // call `nft_tokens` with starting index
  assertTokensAre(
    test,
    await store.view("nft_tokens", { from_index: "2" }),
    bobTokenIds.map((id) => ({ token_id: id, owner_id: bob.accountId })),
    "`nft_tokens({ from_index })` output is wrong"
  );

  // call `nft_tokens` with starting index and limit
  assertTokensAre(
    test,
    await store.view("nft_tokens", { from_index: "1", limit: 2 }),
    [
      { token_id: aliceTokenIds[1], owner_id: alice.accountId },
      { token_id: bobTokenIds[0], owner_id: bob.accountId },
    ],
    "`nft_tokens({ from_index, limit })` output is wrong"
  );

  // call `nft_tokens_for_owner` for Bob without params
  assertTokensAre(
    test,
    await store.view("nft_tokens_for_owner", { account_id: bob.accountId }),
    bobTokenIds.map((id) => ({ token_id: id, owner_id: bob.accountId })),
    "`nft_tokens_for_owner({})` output is wrong"
  );

  // call `nft_tokens_for_owner` for Bob with starting index
  assertTokensAre(
    test,
    await store.view("nft_tokens_for_owner", {
      account_id: bob.accountId,
      // TODO::contracts::medium: should this index refer to token_id, or the
      //  index of token for this token owner? -> if token_id, then use "3"
      from_index: "1",
    }),
    [{ token_id: bobTokenIds[1], owner_id: bob.accountId }],
    "`nft_tokens_for_owner({ from_index })` output is wrong"
  );

  // call `nft_tokens_for_owner` for Bob with starting index and limit
  assertTokensAre(
    test,
    await store.view("nft_tokens_for_owner", {
      account_id: bob.accountId,
      // TODO::contracts::medium: should this index refer to token_id, or the
      //  index of token for this token owner? -> if token_id, then use "2"
      from_index: "0",
      // Unlike `nft_tokens`, here the limit behaves according to spec
      // (see above)
      limit: 1,
    }),
    [{ token_id: bobTokenIds[0], owner_id: bob.accountId }],
    "`nft_tokens_for_owner({ from_index, limit })` output is wrong"
  );

  await bob.call(
    store,
    "nft_batch_burn",
    { token_ids: [bobTokenIds[0]] },
    { attachedDeposit: "1" }
  );

  // call `nft_tokens` with a burned token
  assertTokensAre(
    test,
    await store.view("nft_tokens", {}),
    [
      { token_id: aliceTokenIds[0], owner_id: alice.accountId },
      { token_id: aliceTokenIds[1], owner_id: alice.accountId },
      { token_id: bobTokenIds[1], owner_id: bob.accountId },
    ],
    "`nft_tokens({})` output is wrong after burning"
  );

  // call `nft_tokens_for_owner` for Bob with starting index and limit
  assertTokensAre(
    test,
    await store.view("nft_tokens_for_owner", {
      account_id: bob.accountId,
    }),
    [{ token_id: bobTokenIds[1], owner_id: bob.accountId }],
    "`nft_tokens_for_owner({})` output is wrong after burning"
  );
});
