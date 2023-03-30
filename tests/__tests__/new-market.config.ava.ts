import avaTest from "ava";
import { NearAccount } from "near-workspaces";
import {
  nearToYocto,
  nearToBn,
  getBalance,
  diffCheck,
} from "./utils/balances.js";
import setup from "./setup.js";

const test = setup(avaTest);

test("Properly initialized", async (t) => {
  const { newMarket: market, root } = t.context.accounts;

  t.is(await market.view("get_mintbase_cut"), 5000);
  t.is(await market.view("get_fallback_cut"), 250);
  t.is(await market.view("get_owner"), root.accountId);
  t.is(await market.view("get_listing_lock_seconds"), "0");
  t.is(await market.view("get_listing_storage_deposit"), nearToYocto("0.01"));
  t.deepEqual(await market.view("banned_accounts"), []);
});

test("Owner can set config", async (test) => {
  const { root, alice, newMarket: market } = test.context.accounts;

  await root.call(
    market,
    "set_fallback_cut",
    { new_cut: 500 },
    { attachedDeposit: "1" }
  );
  test.is(await market.view("get_fallback_cut"), 500);

  await root.call(
    market,
    "set_mintbase_cut",
    { new_cut: 500 },
    { attachedDeposit: "1" }
  );
  test.is(await market.view("get_mintbase_cut"), 500);

  await root.call(
    market,
    "set_listing_lock_seconds",
    { secs: "60" },
    { attachedDeposit: "1" }
  );
  test.is(await market.view("get_listing_lock_seconds"), "60");

  await root.call(
    market,
    "set_listing_storage_deposit",
    { deposit: nearToYocto("1") },
    { attachedDeposit: "1" }
  );
  test.is(await market.view("get_listing_storage_deposit"), nearToYocto("1"));

  await root.call(
    market,
    "ban",
    { account_id: "evil.near" },
    { attachedDeposit: "1" }
  );
  test.deepEqual(await market.view("banned_accounts"), ["evil.near"]);
  await root.call(
    market,
    "unban",
    { account_id: "evil.near" },
    { attachedDeposit: "1" }
  );
  test.deepEqual(await market.view("banned_accounts"), []);

  await root.call(
    market,
    "add_affiliate",
    { account_id: alice.accountId, cut: 200 },
    { attachedDeposit: "1" }
  );
  test.deepEqual(await market.view("affiliates"), [[alice.accountId, 200]]);
  await root.call(
    market,
    "del_affiliate",
    { account_id: alice.accountId },
    { attachedDeposit: "1" }
  );
  test.deepEqual(await market.view("affiliates"), []);

  await root.call(
    market,
    "set_owner",
    { new_owner: alice.accountId },
    { attachedDeposit: "1" }
  );
  test.is(await market.view("get_owner"), alice.accountId);
});

test("Deposits work", async (test) => {
  const { alice, newMarket: market } = test.context.accounts;
  const assertDeposit = async (account: NearAccount, deposit: string) => {
    test.is(
      await market.view("get_storage_deposit", { account: account.accountId }),
      deposit
    );
  };

  await assertDeposit(alice, "0");
  const initBalance = await getBalance(alice);

  await alice.call(
    market,
    "deposit_storage",
    {},
    { attachedDeposit: nearToYocto("1") as string }
  );
  await assertDeposit(alice, nearToYocto("1") as string);
  const postDepositBalance = await getBalance(alice);
  test.true(
    diffCheck(
      postDepositBalance,
      initBalance,
      nearToBn("1").neg(), // 1 NEAR gone from alice's account
      nearToBn("0.01") // TODO: why are the gas costs that high?
    ),
    "Deposit did not subtract from balance"
  );

  await alice.call(
    market,
    "claim_unused_storage_deposit",
    {},
    { attachedDeposit: "1" }
  );
  await assertDeposit(alice, "0");
  const postWithdrawBalance = await getBalance(alice);
  test.true(
    diffCheck(
      postWithdrawBalance,
      postDepositBalance,
      nearToBn("1"), // 1 NEAR added to alice's account
      nearToBn("0.01") // TODO: why are the gas costs that high?
    ),
    "Claimed deposit was not refunded"
  );
});
