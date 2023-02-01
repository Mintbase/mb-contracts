import avaTest, { ExecutionContext } from "ava";
import { BN, Gas, NearAccount } from "near-workspaces";
import {
  nearToYocto,
  getBalance,
  diffCheck,
  nearToBn,
  mintingDeposit,
} from "./utils/balances.js";
import { getPanic } from "./utils/panics.js";
import { getEvent } from "./utils/events.js";
import { createPayouts } from "./utils/payouts.js";
import setup, { createAndDeploy } from "./setup.js";

const test = setup(avaTest);

const deployWnear = async (root: NearAccount): Promise<NearAccount> =>
  createAndDeploy(root, "wnear", {
    initialBalanceNear: "10",
    codePath: "../wasm/wnear.wasm",
    initMethod: "new",
    initArgs: {},
  });
const mintAndList = async ({
  alice,
  market,
  store,
  wnear,
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
      msg: JSON.stringify({
        price: nearToYocto("1"),
        ft_contract: wnear.accountId,
      }),
    },
    {
      attachedDeposit: nearToYocto("0.008") as string,
      gas: Gas.parse("50 Tgas"),
    }
  );
};

const wrapNear = async ({
  account,
  wnear,
  amount,
}: {
  account: NearAccount;
  wnear: NearAccount;
  amount: string;
}) => {
  await account.call(
    wnear,
    "near_deposit",
    {},
    { attachedDeposit: nearToYocto(amount) as string }
  );
};

const getWnearBalance = async ({
  account,
  wnear,
}: {
  account: NearAccount;
  wnear: NearAccount;
}): Promise<BN> =>
  new BN(await wnear.view("ft_balance_of", { account_id: account.accountId }));

test("Offers in NEAR are rejected (FT)", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  await mintAndList({ alice, market, store, wnear });

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
    `Smart contract panicked: This NFT is not listed for NEAR, you must instead use \`ft_transfer_call\` on \`${wnear.accountId}\``
  );

  const postMarketBalance = await getBalance(market);
  const postAliceBalance = await getBalance(alice);
  const postBobBalance = await getBalance(bob);

  test.true(
    diffCheck(
      postMarketBalance,
      preMarketBalance,
      nearToBn("0.001"),
      nearToBn("0.001")
    ),
    `preMarketBalance: ${preMarketBalance}, postMarketBalance: ${postMarketBalance}`
  );
  test.true(
    preAliceBalance.eq(postAliceBalance),
    `preAliceBalance: ${preAliceBalance}, postAliceBalance: ${postAliceBalance}`
  );
  test.true(
    diffCheck(postBobBalance, preBobBalance, new BN("0"), nearToBn("0.01")),
    `preBobBalance: ${preBobBalance}, postBobBalance: ${postBobBalance}`
  );
});

test("Offers in wrong FT are rejected (FT)", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  const getOwner = async ({ token_id }: { token_id: string }) =>
    ((await store.view("nft_token", { token_id })) as { owner_id: string })
      .owner_id;
  await mintAndList({ alice, market, store, wnear });
  const wnear2 = await createAndDeploy(root, "wnear2", {
    codePath: "../wasm/wnear.wasm",
    initMethod: "new",
    initArgs: {},
    initialBalanceNear: "2",
  });
  await wrapNear({ account: alice, wnear: wnear2, amount: "0.5" });
  await wrapNear({ account: market, wnear: wnear2, amount: "0.5" });
  await wrapNear({ account: bob, wnear: wnear2, amount: "3" });

  const preAliceBalance = await getWnearBalance({
    account: alice,
    wnear: wnear2,
  });
  const preMarketBalance = await getWnearBalance({
    account: market,
    wnear: wnear2,
  });
  const preBobBalance = await getWnearBalance({
    account: bob,
    wnear: wnear2,
  });

  test.is(await getOwner({ token_id: "0" }), alice.accountId);
  const buyCall = await bob.callRaw(
    wnear2,
    "ft_transfer_call",
    {
      receiver_id: market.accountId,
      amount: nearToYocto("0.5"),
      msg: JSON.stringify({
        nft_contract_id: store.accountId,
        token_id: "0",
      }),
    },
    { attachedDeposit: "1", gas: Gas.parse("299 Tgas") }
  );
  test.is(buyCall.logs.length, 3);
  test.is(
    buyCall.logs[1],
    `This NFT can only be bought with FTs from ${wnear2.accountId}, refunding.`
  );
  test.is(await getOwner({ token_id: "0" }), alice.accountId);

  const postAliceBalance = await getWnearBalance({
    account: alice,
    wnear: wnear2,
  });
  const postMarketBalance = await getWnearBalance({
    account: market,
    wnear: wnear2,
  });
  const postBobBalance = await getWnearBalance({
    account: bob,
    wnear: wnear2,
  });

  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  test.true(postAliceBalance.eq(preAliceBalance));
  test.true(postMarketBalance.eq(preMarketBalance));
  test.true(postBobBalance.eq(preBobBalance));
});

test("Offers below ask are rejected (FT)", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  const getOwner = async ({ token_id }: { token_id: string }) =>
    ((await store.view("nft_token", { token_id })) as { owner_id: string })
      .owner_id;
  await mintAndList({ alice, market, store, wnear });
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "3" });

  const preAliceBalance = await getWnearBalance({ account: alice, wnear });
  const preMarketBalance = await getWnearBalance({ account: market, wnear });
  const preBobBalance = await getWnearBalance({ account: bob, wnear });

  test.is(await getOwner({ token_id: "0" }), alice.accountId);
  const buyCall = await bob.callRaw(
    wnear,
    "ft_transfer_call",
    {
      receiver_id: market.accountId,
      amount: nearToYocto("0.5"),
      msg: JSON.stringify({
        nft_contract_id: store.accountId,
        token_id: "0",
      }),
    },
    { attachedDeposit: "1", gas: Gas.parse("299 Tgas") }
  );
  test.is(buyCall.logs.length, 3);
  test.is(
    buyCall.logs[1],
    "You have not supplied sufficient funds to buy this token, refunding."
  );
  test.is(await getOwner({ token_id: "0" }), alice.accountId);

  const postAliceBalance = await getWnearBalance({ account: alice, wnear });
  const postMarketBalance = await getWnearBalance({ account: market, wnear });
  const postBobBalance = await getWnearBalance({ account: bob, wnear });

  // test.log(`root: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`alice: ${preBobBalance} -> ${postBobBalance}`);
  test.true(postAliceBalance.eq(preAliceBalance));
  test.true(postMarketBalance.eq(preMarketBalance));
  test.true(postBobBalance.eq(preBobBalance));
});

test("Offers above ask are executed (FT)", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  const getOwner = async ({ token_id }: { token_id: string }) =>
    ((await store.view("nft_token", { token_id })) as { owner_id: string })
      .owner_id;
  await mintAndList({ alice, market, store, wnear });
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "3" });

  const preAliceBalance = await getWnearBalance({ account: alice, wnear });
  const preMarketBalance = await getWnearBalance({ account: market, wnear });
  const preBobBalance = await getWnearBalance({ account: bob, wnear });

  test.is(await getOwner({ token_id: "0" }), alice.accountId);
  const buyCall = await bob.callRaw(
    wnear,
    "ft_transfer_call",
    {
      receiver_id: market.accountId,
      amount: nearToYocto("2"),
      msg: JSON.stringify({
        nft_contract_id: store.accountId,
        token_id: "0",
      }),
    },
    { attachedDeposit: "1", gas: Gas.parse("299 Tgas") }
  );
  test.is(buyCall.logs.length, 5);
  test.deepEqual(getEvent(buyCall.logs[1]), {
    standard: "mb_market",
    version: "0.2.1",
    event: "nft_make_offer",
    data: {
      nft_contract_id: store.accountId,
      nft_token_id: "0",
      nft_approval_id: 0,
      offer_id: 0,
      offerer_id: bob.accountId,
      currency: `ft::${wnear.accountId}`,
      price: nearToYocto("2"),
      referrer_id: null,
      referral_amount: null,
    },
  });

  const payout: Record<string, string> = {};
  // const payout = {};
  payout[alice.accountId] = nearToYocto("1.95") as string;
  test.deepEqual(getEvent(buyCall.logs[3]), {
    standard: "mb_market",
    version: "0.2.2",
    event: "nft_sale",
    data: {
      nft_contract_id: store.accountId,
      nft_token_id: "0",
      nft_approval_id: 0,
      accepted_offer_id: 0,
      currency: `ft::${wnear.accountId}`,
      payout,
      price: nearToYocto("2"),
      referrer_id: null,
      referral_amount: null,
      mintbase_amount: "50000000000000000000000",
    },
  });
  test.is(await getOwner({ token_id: "0" }), bob.accountId);

  const postAliceBalance = await getWnearBalance({ account: alice, wnear });
  const postMarketBalance = await getWnearBalance({ account: market, wnear });
  const postBobBalance = await getWnearBalance({ account: bob, wnear });

  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("1.95"))));
  test.true(postMarketBalance.eq(preMarketBalance.add(nearToBn("0.05"))));
  test.true(postBobBalance.eq(preBobBalance.sub(nearToBn("2"))));
});

// // ----------------------- checking referral support ------------------------ //
test("Affiliations work (FT)", async (test) => {
  const {
    root,
    alice,
    bob,
    carol,
    newMarket: market,
    store,
  } = test.context.accounts;
  const wnear = await deployWnear(root);

  await mintAndList({ alice, market, store, wnear });
  await root.call(
    market,
    "add_affiliate",
    { account_id: bob.accountId, cut: 200 },
    { attachedDeposit: "1" }
  );
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "0.5" });
  await wrapNear({ account: carol, wnear, amount: "3" });

  const preAliceBalance = await getWnearBalance({ account: alice, wnear });
  const preMarketBalance = await getWnearBalance({ account: market, wnear });
  const preBobBalance = await getWnearBalance({ account: bob, wnear });
  const preCarolBalance = await getWnearBalance({ account: carol, wnear });

  await carol.call(
    wnear,
    "ft_transfer_call",
    {
      receiver_id: market.accountId,
      amount: nearToYocto("2"),
      msg: JSON.stringify({
        nft_contract_id: store.accountId,
        token_id: "0",
        affiliate_id: bob.accountId,
      }),
    },
    { attachedDeposit: "1", gas: Gas.parse("299 Tgas") }
  );

  const postAliceBalance = await getWnearBalance({ account: alice, wnear });
  const postMarketBalance = await getWnearBalance({ account: market, wnear });
  const postBobBalance = await getWnearBalance({ account: bob, wnear });
  const postCarolBalance = await getWnearBalance({ account: carol, wnear });

  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  // test.log(`carol: ${preCarolBalance} -> ${postCarolBalance}`);
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("1.96"))));
  test.true(postMarketBalance.eq(preMarketBalance.add(nearToBn("0.02"))));
  test.true(postBobBalance.eq(preBobBalance.add(nearToBn("0.02"))));
  test.true(postCarolBalance.eq(preCarolBalance.sub(nearToBn("2"))));
});

// // ---------------------------- checking payouts ---------------------------- //

test("Payouts are respected (FT)", async (test) => {
  const {
    root,
    alice,
    bob,
    carol,
    newMarket: market,
    store,
  } = test.context.accounts;
  const wnear = await deployWnear(root);

  await mintAndList({ alice, market, store, wnear });
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
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "0.5" });
  await wrapNear({ account: carol, wnear, amount: "3" });

  const preAliceBalance = await getWnearBalance({ account: alice, wnear });
  const preMarketBalance = await getWnearBalance({ account: market, wnear });
  const preBobBalance = await getWnearBalance({ account: bob, wnear });
  const preCarolBalance = await getWnearBalance({ account: carol, wnear });

  const buyCall = await carol.callRaw(
    wnear,
    "ft_transfer_call",
    {
      receiver_id: market.accountId,
      amount: nearToYocto("2"),
      msg: JSON.stringify({
        nft_contract_id: store.accountId,
        token_id: "0",
      }),
    },
    { attachedDeposit: "1", gas: Gas.parse("299 Tgas") }
  );

  const postAliceBalance = await getWnearBalance({ account: alice, wnear });
  const postMarketBalance = await getWnearBalance({ account: market, wnear });
  const postBobBalance = await getWnearBalance({ account: bob, wnear });
  const postCarolBalance = await getWnearBalance({ account: carol, wnear });

  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  // test.log(`carol: ${preCarolBalance} -> ${postCarolBalance}`);
  test.true(postAliceBalance.eq(preAliceBalance.add(nearToBn("1.17"))));
  test.true(postMarketBalance.eq(preMarketBalance.add(nearToBn("0.05"))));
  test.true(postBobBalance.eq(preBobBalance.add(nearToBn("0.78"))));
  test.true(postCarolBalance.eq(preCarolBalance.sub(nearToBn("2"))));
});

// // -------------------------- checking edge cases --------------------------- //
// // TODO: move some of the edge cases down here
// // TODO: check logs for refund reasoning
const checkFailedBuy = async (
  test: ExecutionContext,
  { alice, bob, market, store, wnear }: Record<string, NearAccount>
) => {
  const getOwner = async ({ token_id }: { token_id: string }) =>
    ((await store.view("nft_token", { token_id })) as { owner_id: string })
      .owner_id;

  const preAliceBalance = await getWnearBalance({ account: alice, wnear });
  const preMarketBalance = await getWnearBalance({ account: market, wnear });
  const preBobBalance = await getWnearBalance({ account: bob, wnear });
  const preOwner = await getOwner({ token_id: "0" });

  await bob.call(
    wnear,
    "ft_transfer_call",
    {
      receiver_id: market.accountId,
      amount: nearToYocto("2"),
      msg: JSON.stringify({
        nft_contract_id: store.accountId,
        token_id: "0",
      }),
    },
    { attachedDeposit: "1", gas: Gas.parse("299 Tgas") }
  );

  // owner did not change
  const postOwner = await getOwner({ token_id: "0" });
  test.is(preOwner, postOwner);

  const postAliceBalance = await getWnearBalance({ account: alice, wnear });
  const postMarketBalance = await getWnearBalance({ account: market, wnear });
  const postBobBalance = await getWnearBalance({ account: bob, wnear });

  // test.log(`alice: ${preAliceBalance} -> ${postAliceBalance}`);
  // test.log(`market: ${preMarketBalance} -> ${postMarketBalance}`);
  // test.log(`bob: ${preBobBalance} -> ${postBobBalance}`);
  test.true(postAliceBalance.eq(preAliceBalance));
  test.true(postMarketBalance.eq(preMarketBalance));
  test.true(postBobBalance.eq(preBobBalance));
};

test("Non-market transfers lead to graceful failures", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  await mintAndList({ alice, market, store, wnear });
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "3" });

  await alice.call(
    store,
    "nft_transfer",
    { token_id: "0", receiver_id: bob.accountId },
    { attachedDeposit: "1" }
  );

  await checkFailedBuy(test, { alice, bob, market, store, wnear });
});

test("Revoking approvals lead to graceful failures", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  await mintAndList({ alice, market, store, wnear });
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "3" });

  await alice.call(
    store,
    "nft_revoke",
    { token_id: "0", account_id: market.accountId },
    { attachedDeposit: "1" }
  );

  await checkFailedBuy(test, { alice, bob, market, store, wnear });
});

test("Badly updated approvals lead to graceful failures", async (test) => {
  const { root, alice, bob, newMarket: market, store } = test.context.accounts;
  const wnear = await deployWnear(root);

  await mintAndList({ alice, market, store, wnear });
  await wrapNear({ account: alice, wnear, amount: "0.5" });
  await wrapNear({ account: market, wnear, amount: "0.5" });
  await wrapNear({ account: bob, wnear, amount: "3" });

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

  await checkFailedBuy(test, { alice, bob, market, store, wnear });
});
