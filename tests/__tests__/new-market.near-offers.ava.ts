import avaTest, { ExecutionContext } from "ava";
import { BN, Gas, NearAccount } from "near-workspaces";
import {
  nearToYocto,
  getBalance,
  diffCheck,
  nearToBn,
  mintingDeposit,
} from "./utils/balances.js";
import { createPayouts } from "./utils/payouts.js";
import { getPanic } from "./utils/panics.js";
import setup from "./setup.js";

const test = setup(avaTest);

const mintAndList = async ({
  alice,
  market,
  store,
}: Record<string, NearAccount>) => {
  await alice.call(
    store,
    "nft_batch_mint",
    { owner_id: alice, metadata: {}, num_to_mint: 1 },
    { attachedDeposit: mintingDeposit({ n_tokens: 1 }) }
  );

  await alice.call(
    market,
    "deposit_storage",
    {},
    { attachedDeposit: nearToYocto("0.01") as string }
  );

  await alice.call(
    store,
    "nft_approve",
    {
      token_id: "0",
      account_id: market.accountId,
      msg: JSON.stringify({ price: nearToYocto("1") }),
    },
    {
      attachedDeposit: nearToYocto("0.008") as string,
      gas: Gas.parse("50 Tgas"),
    }
  );
};

test("Offers below ask are rejected (NEAR)", async (test) => {
  const { alice, bob, newMarket: market, store } = test.context.accounts;

  await mintAndList({ alice, market, store });

  const preMarketBalance = await getBalance(market);
  const preAliceBalance = await getBalance(alice);
  const preBobBalance = await getBalance(bob);

  const buyCall = await bob.callRaw(
    market,
    "buy",
    { nft_contract_id: store.accountId, token_id: "0" },
    { attachedDeposit: nearToYocto("0.9") as string }
  );
  test.is(
    getPanic(buyCall),
    "Smart contract panicked: Deposit needs to be higher than listing price"
  );

  const postMarketBalance = await getBalance(market);
  const postAliceBalance = await getBalance(alice);
  const postBobBalance = await getBalance(bob);

  // market looses some to refund promise?
  test.true(
    diffCheck(
      postMarketBalance,
      preMarketBalance,
      nearToBn("0.001"), // FIXME: where did the market get NEAR?
      nearToBn("0.001")
    ),
    `preMarketBalance: ${preMarketBalance}, postMarketBalance: ${postMarketBalance}`
  );
  test.true(
    preAliceBalance.eq(postAliceBalance),
    `preRootBalance: ${preAliceBalance}, postRootBalance: ${postMarketBalance}`
  );
  test.true(
    diffCheck(postBobBalance, preBobBalance, new BN("0"), nearToBn("0.01")),
    `preAliceBalance: ${preBobBalance}, postAliceBalance: ${postBobBalance}`
  );
});

// test("Offers above ask are executed (NEAR)", async (test) => {
//   const { root, alice, market, store } = test.context.accounts;

//   const getOwner = async ({ token_id }) =>
//     ((await store.view("nft_token", { token_id })) as { owner_id: string })
//       .owner_id;
//   await mintAndList({ root, market, store });

//   const preRootBalance = await getBalance(root);
//   const preMarketBalance = await getBalance(market);
//   const preAliceBalance = await getBalance(alice);

//   test.is(await getOwner({ token_id: "0" }), root.accountId);
//   const buyCall = await alice.callRaw(
//     market,
//     "buy",
//     { nft_contract_id: store.accountId, token_id: "0" },
//     { attachedDeposit: nearToYocto("2") as string, gas: Gas.parse("225 Tgas") }
//   );
//   test.is(buyCall.logs.length, 3);
//   test.deepEqual(getEvent(buyCall.logs[0]), {
//     standard: "mb_market",
//     version: "0.2.1",
//     event: "nft_make_offer",
//     data: {
//       nft_contract_id: store.accountId,
//       nft_token_id: "0",
//       nft_approval_id: 0,
//       offer_id: 0,
//       offerer_id: alice.accountId,
//       currency: "near",
//       price: nearToYocto("2"),
//       referrer_id: null,
//       referral_amount: null,
//     },
//   });

//   const payout = {};
//   payout[root.accountId] = nearToYocto("1.95");
//   test.deepEqual(getEvent(buyCall.logs[2]), {
//     standard: "mb_market",
//     version: "0.2.2",
//     event: "nft_sale",
//     data: {
//       nft_contract_id: store.accountId,
//       nft_token_id: "0",
//       nft_approval_id: 0,
//       accepted_offer_id: 0,
//       payout,
//       currency: "near",
//       price: nearToYocto("2"),
//       referrer_id: null,
//       referral_amount: null,
//       mintbase_amount: "50000000000000000000000",
//     },
//   });
//   test.is(await getOwner({ token_id: "0" }), alice.accountId);

//   const postRootBalance = await getBalance(root);
//   const postMarketBalance = await getBalance(market);
//   const postAliceBalance = await getBalance(alice);

//   // 1.95 revenue + 0.01 storage refund
//   test.true(postRootBalance.eq(preRootBalance.add(nearToBn("1.96"))));
//   // TODO: find out why this difference is not exact 0.04
//   test.true(
//     diffCheck(
//       postMarketBalance,
//       preMarketBalance,
//       nearToBn("0.05"),
//       nearToBn("0.01") // this gas might be due to transfers
//     )
//   );
//   test.true(
//     diffCheck(
//       postAliceBalance,
//       preAliceBalance,
//       nearToBn("2").neg(),
//       nearToBn("0.05")
//     )
//   );
// });

// // ----------------------- checking referral support ------------------------ //
test("Affiliations work (NEAR)", async (test) => {
  const {
    root,
    alice,
    bob,
    carol,
    newMarket: market,
    store,
  } = test.context.accounts;
  await mintAndList({ alice, market, store });
  await root.call(
    market,
    "add_affiliate",
    { account_id: bob.accountId, cut: 200 },
    { attachedDeposit: "1" }
  );

  const preMarketBalance = await getBalance(market);
  const preAliceBalance = await getBalance(alice);
  const preBobBalance = await getBalance(bob);
  const preCarolBalance = await getBalance(carol);

  await carol.call(
    market,
    "buy",
    {
      nft_contract_id: store.accountId,
      token_id: "0",
      affiliate_id: bob.accountId,
    },
    { attachedDeposit: nearToYocto("10") as string, gas: Gas.parse("225 Tgas") }
  );

  const postMarketBalance = await getBalance(market);
  const postAliceBalance = await getBalance(alice);
  const postBobBalance = await getBalance(bob);
  const postCarolBalance = await getBalance(carol);

  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  // test.log(`carol: ${preCarolBalance} -> ${postCarolBalance}`);
  // Market should loose the storage deposit and gain its fee
  test.true(
    diffCheck(
      postMarketBalance,
      preMarketBalance,
      nearToBn("0.10"),
      nearToBn("0.01") // -> storage + yocto + something else (not exact)
    )
  );
  // 9.80 for the sale, 0.01 storage refund
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("9.81"))));
  // Bob get's 1
  test.true(postBobBalance.eq(preBobBalance.add(nearToBn("0.1"))));
  test.true(
    diffCheck(
      postCarolBalance,
      preCarolBalance,
      nearToBn("10").neg(),
      nearToBn("0.05")
    )
  );
});

test("Affiliations work (NEAR, referrer interface)", async (test) => {
  const {
    root,
    alice,
    bob,
    carol,
    newMarket: market,
    store,
  } = test.context.accounts;
  await mintAndList({ alice, market, store });
  await root.call(
    market,
    "add_affiliate",
    { account_id: bob.accountId, cut: 200 },
    { attachedDeposit: "1" }
  );

  const preMarketBalance = await getBalance(market);
  const preAliceBalance = await getBalance(alice);
  const preBobBalance = await getBalance(bob);
  const preCarolBalance = await getBalance(carol);

  await carol.call(
    market,
    "buy",
    {
      nft_contract_id: store.accountId,
      token_id: "0",
      referrer_id: bob.accountId,
    },
    { attachedDeposit: nearToYocto("10") as string, gas: Gas.parse("225 Tgas") }
  );

  const postMarketBalance = await getBalance(market);
  const postAliceBalance = await getBalance(alice);
  const postBobBalance = await getBalance(bob);
  const postCarolBalance = await getBalance(carol);

  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  // test.log(`carol: ${preCarolBalance} -> ${postCarolBalance}`);
  // Market should loose the storage deposit and gain its fee
  test.true(
    diffCheck(
      postMarketBalance,
      preMarketBalance,
      nearToBn("0.10"),
      nearToBn("0.01") // -> storage + yocto + something else (not exact)
    )
  );
  // 9.80 for the sale, 0.01 storage refund
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("9.81"))));
  // Bob get's 1
  test.true(postBobBalance.eq(preBobBalance.add(nearToBn("0.1"))));
  test.true(
    diffCheck(
      postCarolBalance,
      preCarolBalance,
      nearToBn("10").neg(),
      nearToBn("0.05")
    )
  );
});

// // ---------------------------- checking payouts ---------------------------- //

test("Payouts are respected (NEAR)", async (test) => {
  const { alice, bob, carol, newMarket: market, store } = test.context.accounts;

  await mintAndList({ alice, market, store });
  await alice.call(
    store,
    "set_split_owners",
    {
      token_ids: ["0"],
      split_between: createPayouts([
        [alice, 6000],
        [bob, 4000],
      ]),
    },
    { attachedDeposit: nearToYocto("0.0016") as string }
  );

  const preMarketBalance = await getBalance(market);
  const preAliceBalance = await getBalance(alice);
  const preBobBalance = await getBalance(bob);
  const preCarolBalance = await getBalance(carol);

  await carol.call(
    market,
    "buy",
    { nft_contract_id: store.accountId, token_id: "0" },
    { attachedDeposit: nearToYocto("10") as string, gas: Gas.parse("225 Tgas") }
  );

  const postMarketBalance = await getBalance(market);
  const postAliceBalance = await getBalance(alice);
  const postBobBalance = await getBalance(bob);
  const postCarolBalance = await getBalance(carol);

  test.true(
    diffCheck(
      postMarketBalance,
      preMarketBalance,
      nearToBn("0.25"),
      nearToBn("0.01") // -> storage + yocto + something else (not exact)
    )
  );
  // 5.85 for the sale, 0.01 storage refund
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("5.86"))));
  test.true(postBobBalance.eq(preBobBalance.add(nearToBn("3.9"))));
  test.true(
    diffCheck(
      postCarolBalance,
      preCarolBalance,
      nearToBn("10").neg(),
      nearToBn("0.05")
    )
  );
});

// // -------------------------- checking edge cases --------------------------- //
const checkFailedBuy = async (
  test: ExecutionContext,
  { alice, bob, market, store }: Record<string, NearAccount>
) => {
  const getOwner = async ({
    token_id,
  }: {
    token_id: string;
  }): Promise<string> =>
    ((await store.view("nft_token", { token_id })) as { owner_id: string })
      .owner_id;

  const preMarketBalance = await getBalance(market);
  const preAliceBalance = await getBalance(alice);
  const preBobBalance = await getBalance(bob);
  const preOwner = await getOwner({ token_id: "0" });
  await bob.call(
    market,
    "buy",
    { nft_contract_id: store.accountId, token_id: "0" },
    { attachedDeposit: nearToYocto("2") as string, gas: Gas.parse("225 Tgas") }
  );
  // owner did not change
  const postOwner = await getOwner({ token_id: "0" });
  test.is(preOwner, postOwner);
  // alice should have gotten her deposit back
  const postAliceBalance = await getBalance(alice);
  test.log();
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("0.01"))));
  // market should have lost 0.01 for deposit refund
  const postMarketBalance = await getBalance(market);
  // TODO: find out why this difference is not exactly 0.01
  // test.true(postMarketBalance.eq(preMarketBalance.sub(nearToBn("0.01"))));
  test.true(
    diffCheck(
      postMarketBalance,
      preMarketBalance,
      new BN("0"),
      nearToBn("0.01")
    )
  );
  // alice should be unchanged, but she paid for gas
  const postBobBalance = await getBalance(bob);
  // test.log(`preBobBalance: ${preBobBalance}`);
  // test.log(`postBobBalance: ${postBobBalance}`);
  test.true(
    diffCheck(postBobBalance, preBobBalance, new BN("0"), nearToBn("0.05"))
  );
};

test("Non-market transfers lead to graceful failures (NEAR)", async (test) => {
  const { alice, bob, newMarket: market, store } = test.context.accounts;

  await mintAndList({ alice, market, store });

  await alice.call(
    store,
    "nft_transfer",
    { token_id: "0", receiver_id: bob.accountId },
    { attachedDeposit: "1" }
  );

  await checkFailedBuy(test, { alice, bob, market, store });
});

test("Revoking approvals lead to graceful failures (NEAR)", async (test) => {
  const { alice, bob, newMarket: market, store } = test.context.accounts;

  await mintAndList({ alice, market, store });

  await alice.call(
    store,
    "nft_revoke",
    { token_id: "0", account_id: market.accountId },
    { attachedDeposit: "1" }
  );

  await checkFailedBuy(test, { alice, bob, market, store });
});

test("Badly updated approvals lead to graceful failures (NEAR)", async (test) => {
  const { alice, bob, newMarket: market, store } = test.context.accounts;

  await mintAndList({ alice, market, store });

  await alice.call(
    store,
    "nft_revoke",
    { token_id: "0", account_id: market.accountId },
    { attachedDeposit: "1" }
  );
  await alice.call(
    store,
    "nft_approve",
    // no msg -> market never get's a callback
    { token_id: "0", account_id: market.accountId },
    {
      attachedDeposit: nearToYocto("0.008") as string,
      gas: Gas.parse("50 Tgas"),
    }
  );

  await checkFailedBuy(test, { alice, bob, market, store });
});
