import avaTest from "ava";
import { Gas } from "near-workspaces";
import {
  nearToYocto,
  getBalance,
  diffCheck,
  nearToBn,
} from "./utils/balances.js";
import { getEvent } from "./utils/events.js";
import { getPanic } from "./utils/panics.js";
import setup from "./setup.js";
import { batchMint, getTokenIds } from "./utils/index.js";

const test = setup(avaTest);

test("interop-market::create-listing", async (test) => {
  const { alice, bob, newMarket: market, store } = test.context.accounts;

  const mintCall = await batchMint({ owner: alice, store, num_to_mint: 1 });
  const tokenId = getTokenIds(mintCall)[0];

  await alice.call(
    market,
    "deposit_storage",
    {},
    { attachedDeposit: nearToYocto("0.01") as string }
  );

  // check that listings are created
  const approveCall = await alice.callRaw(
    store,
    "nft_approve",
    {
      token_id: tokenId,
      account_id: market.accountId,
      msg: JSON.stringify({ price: nearToYocto("1") }),
    },
    {
      attachedDeposit: nearToYocto("0.008") as string,
      gas: Gas.parse("50 Tgas"),
    }
  );
  // check event
  test.is(approveCall.logs.length, 2); // 0 is approval event from NFT contract
  test.deepEqual(getEvent(approveCall.logs[1]), {
    standard: "mb_market",
    version: "0.2.1",
    event: "nft_list",
    data: {
      kind: "simple",
      nft_contract_id: store.accountId,
      nft_token_id: tokenId,
      nft_approval_id: 0,
      nft_owner_id: alice.accountId,
      currency: "near",
      price: nearToYocto("1"),
    },
  });

  test.like(
    await market.view("get_listing", {
      nft_contract_id: store.accountId,
      token_id: tokenId,
    }),
    {
      nft_token_id: tokenId,
      nft_contract_id: store.accountId,
      nft_approval_id: 0,
      nft_owner_id: alice.accountId,
      price: nearToYocto("1"),
      current_offer: null,
    }
  );

  // check that no withdrawal happens
  const preWithdrawBalance = await getBalance(alice);
  await alice.call(
    market,
    "claim_unused_storage_deposit",
    {},
    { attachedDeposit: "1" }
  );
  const postWithdrawBalance = await getBalance(alice);
  test.true(
    diffCheck(
      postWithdrawBalance,
      preWithdrawBalance,
      nearToBn("0"),
      nearToBn("0.01") // TODO: why are the gas costs that high?
    ),
    "User claimed required deposit"
  );

  // bob cannot unlist for root
  const unlistCallBob = await bob.callRaw(
    market,
    "unlist",
    { nft_contract_id: store.accountId, token_ids: [tokenId] },
    { attachedDeposit: "1" }
  );
  test.is(
    getPanic(unlistCallBob),
    `Smart contract panicked: Only ${alice.accountId} is allowed to call this!`
  );

  // yocto deposit required to unlist
  const unlistCallNoYocto = await alice.callRaw(market, "unlist", {
    nft_contract_id: store.accountId,
    token_ids: [tokenId],
  });
  test.is(
    getPanic(unlistCallNoYocto),
    `Smart contract panicked: Requires attached deposit of exactly 1 yoctoNEAR`
  );

  // check unlisting
  const preUnlistBalance = await getBalance(alice);
  const unlistCall = await alice.callRaw(
    market,
    "unlist",
    { nft_contract_id: store.accountId, token_ids: [tokenId] },
    { attachedDeposit: "1" }
  );
  // check event
  test.is(unlistCall.logs.length, 1);
  test.deepEqual(getEvent(unlistCall.logs[0]), {
    standard: "mb_market",
    version: "0.2.1",
    event: "nft_unlist",
    data: {
      nft_contract_id: store.accountId,
      nft_token_id: tokenId,
      nft_approval_id: 0,
    },
  });

  const postUnlistBalance = await getBalance(alice);
  test.true(
    diffCheck(
      preUnlistBalance,
      postUnlistBalance,
      nearToBn("0.01"),
      nearToBn("0.02") // TODO: why are the gas costs that high?
    ),
    `preUnlistBalance: ${preUnlistBalance}, postUnlistBalance: ${postUnlistBalance}`
  );
});

test("interop-market::listing-deposit", async (test) => {
  const { alice, newMarket: market, store } = test.context.accounts;

  const mintCall = await batchMint({ owner: alice, store, num_to_mint: 1 });
  const tokenId = getTokenIds(mintCall)[0];

  const approveCall = await alice.callRaw(
    store,
    "nft_approve",
    {
      token_id: tokenId,
      account_id: market.accountId,
      msg: JSON.stringify({ price: nearToYocto("1") }),
    },
    {
      attachedDeposit: nearToYocto("0.008") as string,
      gas: Gas.parse("50 Tgas"),
    }
  );
  test.is(
    getPanic(approveCall),
    "Smart contract panicked: Storage for listing not covered"
  );
});
